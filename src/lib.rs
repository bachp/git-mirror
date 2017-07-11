/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::process::{Command, Stdio, exit};
use std::fs;
use std::path::Path;

// Used for error and debug logging
#[macro_use]
extern crate log;
use log::LogLevel::Info;

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

use provider::{Mirror, Provider};


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
        if !log_enabled!(Info) {
            git.stdout(Stdio::null());
            git.stderr(Stdio::null());
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

    fs::DirBuilder::new()
        .recursive(true)
        .create(&mirror_dir)
        .or_else(|e| {
            Err(format!(
                "Unable to create mirror dir: {:?} ({})",
                mirror_dir,
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

fn run_sync_task(v: Vec<Mirror>, worker_count: usize, mirror_dir: &str, dry_run: bool) {
    // Give the work to the worker pool
    let pool = ThreadPool::new(worker_count);
    let mut n = 0;

    let (tx, rx) = channel();
    for x in v {
        let tx = tx.clone();
        let mirror_dir = mirror_dir.to_owned().clone();
        pool.execute(move || {
            print!("{} -> {} : ", x.origin, x.destination);
            let c = match mirror_repo(mirror_dir, &x.origin, &x.destination, dry_run) {
                Ok(c) => {
                    println!("OK");
                    c
                }
                Err(e) => {
                    println!("ERROR");
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

    println!("Done {0}/{1}", rx.iter().take(n).fold(0, |a, b| a + b), n);

}


pub fn do_mirror(provider: &Provider, worker_count: usize, mirror_dir: &str, dry_run: bool) {

    // Get the list of repos to sync from gitlabsss
    let v = provider.get_mirror_repos().unwrap_or_else(|e| {
        error!("Unable to get mirror repos ({})", e);
        exit(1);
    });

    run_sync_task(v, worker_count, mirror_dir, dry_run);
}


pub mod provider;
