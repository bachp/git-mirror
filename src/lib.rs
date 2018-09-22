/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::fs;
use std::fs::File;
use std::path::Path;

// File locking
extern crate fs2;
use fs2::FileExt;

// Used for error and debug logging
#[macro_use]
extern crate log;

// Used to create sane local directory names
extern crate slug;
use self::slug::slugify;

// We need the header! macro from hyper
extern crate reqwest;

// Macros for serde
#[macro_use]
extern crate serde_derive;

// Used to allow multiple paralell sync tasks
extern crate rayon;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

// Time handling
extern crate chrono;
use chrono::{Local, Utc};

// Monitoring
#[macro_use]
extern crate prometheus;
use prometheus::{Encoder, TextEncoder};

use provider::{MirrorResult, Provider};

use git::{Git, GitWrapper};

pub fn mirror_repo(
    mirror_dir: &str,
    origin: &str,
    destination: &str,
    dry_run: bool,
    git_executable: String,
) -> Result<u32, String> {
    if dry_run {
        return Ok(1);
    }

    let origin_dir = Path::new(&mirror_dir).join(slugify(origin));
    debug!("Using origin dir: {0:?}", origin_dir);

    let git = Git::new(git_executable);

    git.git_version()?;

    if origin_dir.is_dir() {
        info!("Local Update for {}", origin);

        git.git_update_mirror(origin, &origin_dir)?;
    } else if !origin_dir.exists() {
        info!("Local Checkout for {}", origin);

        git.git_clone_mirror(origin, &origin_dir)?;
    } else {
        return Err(format!("Local origin dir is a file: {:?}", origin_dir));
    }

    info!("Push to destination {}", destination);

    git.git_push_mirror(destination, &origin_dir)?;

    Ok(1)
}

fn run_sync_task(
    v: &[MirrorResult],
    worker_count: usize,
    mirror_dir: &str,
    dry_run: bool,
    label: &str,
    git_executable: &str,
) {
    // Give the work to the worker pool
    rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build_global()
        .unwrap();

    let proj_total =
        register_counter_vec!("git_mirror_total", "Total projects", &["mirror"]).unwrap();
    let proj_skip =
        register_counter_vec!("git_mirror_skip", "Skipped projects", &["mirror"]).unwrap();
    let proj_fail =
        register_counter_vec!("git_mirror_fail", "Failed projects", &["mirror"]).unwrap();
    let proj_ok = register_counter_vec!("git_mirror_ok", "OK projects", &["mirror"]).unwrap();
    let proj_start = register_gauge_vec!(
        "git_mirror_project_start",
        "Start of project mirror as unix timestamp",
        &["origin", "destination", "mirror"]
    ).unwrap();
    let proj_end = register_gauge_vec!(
        "git_mirror_project_end",
        "End of projeect mirror as unix timestamp",
        &["origin", "destination", "mirror"]
    ).unwrap();

    let total = v.len();
    let success = v
        .par_iter()
        .map(|x| {
            proj_total.with_label_values(&[label]).inc();
            match x {
                Ok(x) => {
                    let mirror_dir = mirror_dir.to_owned().clone();
                    let proj_fail = proj_fail.clone();
                    let proj_ok = proj_ok.clone();
                    let proj_start = proj_start.clone();
                    let proj_end = proj_end.clone();
                    let label = label.to_string();
                    let git_executable = git_executable.to_string();
                    println!(
                        "START [{}]: {} -> {}",
                        Local::now(),
                        x.origin,
                        x.destination
                    );
                    proj_start
                        .with_label_values(&[&x.origin, &x.destination, &label])
                        .set(Utc::now().timestamp() as f64);
                    match mirror_repo(
                        &mirror_dir,
                        &x.origin,
                        &x.destination,
                        dry_run,
                        git_executable,
                    ) {
                        Ok(c) => {
                            println!("OK [{}]: {} -> {}", Local::now(), x.origin, x.destination);
                            proj_end
                                .with_label_values(&[&x.origin, &x.destination, &label])
                                .set(Utc::now().timestamp() as f64);
                            proj_ok.with_label_values(&[&label]).inc();
                            c
                        }
                        Err(e) => {
                            println!(
                                "FAIL [{}]: {} -> {} ({})",
                                Local::now(),
                                x.origin,
                                x.destination,
                                e
                            );
                            proj_end
                                .with_label_values(&[&x.origin, &x.destination, &label])
                                .set(Utc::now().timestamp() as f64);
                            proj_fail.with_label_values(&[&label]).inc();
                            error!(
                                "Unable to sync repo {} -> {} ({})",
                                x.origin, x.destination, e
                            );
                            0
                        }
                    }
                }
                Err(e) => {
                    proj_skip.with_label_values(&[label]).inc();
                    warn!("Skipping: {:?}", e);
                    0
                }
            }
        }).sum::<u32>();

    println!("DONE [{2}]: {0}/{1}", success, total, Local::now());
}

pub struct MirrorOptions {
    pub dry_run: bool,
    pub metrics_file: Option<String>,
    pub worker_count: usize,
    pub git_executable: String,
}

pub fn do_mirror(
    provider: &Box<Provider>,
    mirror_dir: &str,
    opts: &MirrorOptions,
) -> Result<(), String> {
    let start_time = register_gauge_vec!(
        "git_mirror_start_time",
        "Start time of the sync as unix timestamp",
        &["mirror"]
    ).unwrap();
    let end_time = register_gauge_vec!(
        "git_mirror_end_time",
        "End time of the sync as unix timestamp",
        &["mirror"]
    ).unwrap();

    // Make sure the mirror directory exists
    trace!("Create mirror directory at {:?}", mirror_dir);
    fs::create_dir_all(&mirror_dir)
        .map_err(|e| format!("Unable to create mirror dir: {:?} ({})", &mirror_dir, e))?;

    // Check that only one instance is running against a mirror directory
    let lockfile_path = Path::new(mirror_dir).join("git-mirror.lock");
    let lockfile = fs::File::create(&lockfile_path)
        .map_err(|e| format!("Unable to open lockfile: {:?} ({})", &lockfile_path, e))?;

    lockfile.try_lock_exclusive().map_err(|e| {
        format!(
            "Another instance is already running against the same mirror directory: {:?} ({})",
            &mirror_dir, e
        )
    })?;

    trace!("Aquired lockfile: {:?}", &lockfile);

    // Get the list of repos to sync from gitlabsss
    let v = provider
        .get_mirror_repos()
        .map_err(|e| -> String { format!("Unable to get mirror repos ({})", e) })?;

    start_time
        .with_label_values(&[&provider.get_label()])
        .set(Utc::now().timestamp() as f64);

    run_sync_task(
        &v,
        opts.worker_count,
        mirror_dir,
        opts.dry_run,
        &provider.get_label(),
        &opts.git_executable,
    );

    end_time
        .with_label_values(&[&provider.get_label()])
        .set(Utc::now().timestamp() as f64);

    match opts.metrics_file {
        Some(ref f) => write_metrics(f),
        None => trace!("Skipping metrics file creation"),
    };

    Ok(())
}

fn write_metrics(f: &str) {
    let mut file = File::create(f).unwrap();
    let encoder = TextEncoder::new();
    let metric_familys = prometheus::gather();
    encoder.encode(&metric_familys, &mut file).unwrap();
}

mod git;
pub mod provider;
