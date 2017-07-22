/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::process::{Command, Stdio, exit};
use std::fs;
use std::path::Path;
use std::fs::File;

// File locking
extern crate fs2;
use fs2::FileExt;

// Used for error and debug logging
#[macro_use]
extern crate log;
use log::LogLevel::{Debug, Info};

// Used to create sane local directory names
extern crate slug;
use self::slug::slugify;

// UMacros used for hyper
#[macro_use]
extern crate hyper;

// Macros for serde
#[macro_use]
extern crate serde_derive;

// Used to allow multiple paralell sync tasks
extern crate threadpool;
use threadpool::ThreadPool;
use std::sync::mpsc::channel;

// Time handling
extern crate chrono;
use chrono::{Local, Utc};

// Monitoring
#[macro_use]
extern crate prometheus;
use prometheus::{TextEncoder, Encoder};

use provider::{MirrorResult, Provider};


pub fn mirror_repo(
    mirror_dir: String,
    origin: &str,
    destination: &str,
    dry_run: bool,
) -> Result<u8, String> {

    if dry_run {
        return Ok(1);
    }

    let origin_dir = Path::new(&mirror_dir).join(slugify(origin));
    debug!("Using origin dir: {0:?}", origin_dir);

    // Group common setting for al git commands in this closure
    let git_base_cmd = || {
        let mut git = Command::new("git");
        if !log_enabled!(Debug) {
            git.stdout(Stdio::null());
        }
        debug!("Level {:?}", log_enabled!(Info));
        git.env("GIT_TERMINAL_PROMPT", "0");
        return git;
    };

    git_base_cmd().arg("--version").status().or_else(|e| {
        Err(format!(
            "Unable to execute git --version, make sure git is installed. ({})",
            e
        ))
    })?;

    if origin_dir.is_dir() {
        info!("Local Update for {}", origin);

        let mut set_url_cmd = git_base_cmd();
        set_url_cmd
            .current_dir(&origin_dir)
            .args(&["remote", "set-url", "origin"])
            .arg(origin);

        trace!("Set url command started: {:?}", set_url_cmd);

        let out = set_url_cmd.status().or_else(|e| {
            Err(format!(
                "Unable to execute set url command: {:?} ({})",
                set_url_cmd,
                e
            ))
        })?;

        if !out.success() {
            return Err(format!(
                "Set url command ({:?}) failed with exit code: {}",
                set_url_cmd,
                out
            ));
        }

        let mut remote_update_cmd = git_base_cmd();
        remote_update_cmd.current_dir(&origin_dir).args(
            &[
                "remote",
                "update",
            ],
        );

        trace!("Remote update command started: {:?}", remote_update_cmd);

        let out = remote_update_cmd.status().or_else(|e| {
            Err(format!(
                "Unable to execute remote update command: {:?} ({})",
                remote_update_cmd,
                e
            ))
        })?;

        if !out.success() {
            return Err(format!(
                "Remote update command ({:?}) failed with exit code: {}",
                remote_update_cmd,
                out
            ));
        }

    } else if !origin_dir.exists() {
        info!("Local Checkout for {}", origin);

        let mut clone_cmd = git_base_cmd();
        clone_cmd.args(&["clone", "--mirror"]).arg(origin).arg(
            &origin_dir,
        );

        trace!("Clone command started: {:?}", clone_cmd);

        let out = clone_cmd.status().or_else(|e| {
            Err(format!(
                "Unable to execute clone command: {:?} ({})",
                clone_cmd,
                e
            ))
        })?;

        if !out.success() {
            return Err(format!(
                "Clone command ({:?}) failed with exit code: {}",
                clone_cmd,
                out
            ));
        }

    } else {
        return Err(format!("Local origin dir is a file: {:?}", origin_dir));
    }

    info!("Push to destination {}", destination);

    let mut push_cmd = git_base_cmd();
    push_cmd
        .current_dir(&origin_dir)
        .args(&["push", "--mirror"])
        .arg(destination);

    trace!("Push  started: {:?}", push_cmd);

    let out = push_cmd.status().or_else(|e| {
        Err(format!(
            "Unable to execute push command: {:?} ({})",
            push_cmd,
            e
        ))
    })?;

    if !out.success() {
        return Err(format!(
            "Push command ({:?}) failed with exit code: {}",
            push_cmd,
            out
        ));
    }

    return Ok(1);
}

fn run_sync_task(
    v: Vec<MirrorResult>,
    worker_count: usize,
    mirror_dir: &str,
    dry_run: bool,
    label: String,
) {
    // Give the work to the worker pool
    let pool = ThreadPool::new(worker_count);
    let mut n = 0;

    let proj_total = register_counter_vec!("git_mirror_total", "Total projects", &["mirror"])
        .unwrap();
    let proj_skip = register_counter_vec!("git_mirror_skip", "Skipped projects", &["mirror"])
        .unwrap();
    let proj_fail = register_counter_vec!("git_mirror_fail", "Failed projects", &["mirror"])
        .unwrap();
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

    let (tx, rx) = channel();
    for x in v {
        proj_total.with_label_values(&[&label]).inc();
        match x {
            Ok(x) => {
                let tx = tx.clone();
                let mirror_dir = mirror_dir.to_owned().clone();
                let proj_fail = proj_fail.clone();
                let proj_ok = proj_ok.clone();
                let proj_start = proj_start.clone();
                let proj_end = proj_end.clone();
                let label = label.clone();
                pool.execute(move || {
                    println!(
                        "START [{}]: {} -> {}",
                        Local::now(),
                        x.origin,
                        x.destination
                    );
                    proj_start
                        .with_label_values(&[&x.origin, &x.destination, &label])
                        .set(Utc::now().timestamp() as f64);
                    let c = match mirror_repo(mirror_dir, &x.origin, &x.destination, dry_run) {
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
                                x.origin,
                                x.destination,
                                e
                            );
                            0
                        }
                    };
                    tx.send(c).unwrap();
                });
                n += 1;
            }
            Err(e) => {
                proj_skip.with_label_values(&[&label]).inc();
                warn!("Skipping: {:?}", e);
            }
        };
    }

    println!(
        "DONE [{2}]: {0}/{1}",
        rx.iter().take(n).fold(0, |a, b| a + b),
        n,
        Local::now()
    );

}


pub fn do_mirror(
    provider: &Provider,
    worker_count: usize,
    mirror_dir: &str,
    dry_run: bool,
    metrics_file: Option<String>,
) {
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
    fs::create_dir_all(&mirror_dir).unwrap_or_else(|e| {
        error!("Unable to create mirror dir: {:?} ({})", &mirror_dir, e);
        exit(2);
    });

    // Check that only one instance is running against a mirror directory
    let lockfile_path = Path::new(mirror_dir).join("git-mirror.lock");
    let lockfile = fs::File::create(&lockfile_path).unwrap_or_else(|e| {
        error!("Unable to open lockfile: {:?} ({})", &lockfile_path, e);
        exit(3);
    });

    lockfile.try_lock_exclusive().unwrap_or_else(|e| {
        error!(
            "Another instance is already running aginst the same mirror directory: {:?} ({})",
            &mirror_dir,
            e
        );
        exit(4);
    });

    trace!("Aquired lockfile: {:?}", &lockfile);

    // Get the list of repos to sync from gitlabsss
    let v = provider.get_mirror_repos().unwrap_or_else(|e| {
        error!("Unable to get mirror repos ({})", e);
        exit(1);
    });

    start_time.with_label_values(&[&provider.get_label()]).set(Utc::now().timestamp() as f64);

    run_sync_task(v, worker_count, mirror_dir, dry_run, provider.get_label());

    end_time.with_label_values(&[&provider.get_label()]).set(Utc::now().timestamp() as f64);

    match metrics_file {
        Some(f) => write_metrics(&f),
        None => trace!("Skipping merics file creation"),

    };
}

fn write_metrics(f: &str) {
    let mut file = File::create(f).unwrap();
    let encoder = TextEncoder::new();
    let metric_familys = prometheus::gather();
    encoder.encode(&metric_familys, &mut file).unwrap();
}

pub mod provider;
