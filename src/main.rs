/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::process::{Command, exit};
use std::fs;
use std::path::Path;
use std::env;
use std::cmp;

// Used for error and debug logging
#[macro_use]
extern crate log;
extern crate stderrlog;

// Used to create sane local directory names
extern crate slug;
use self::slug::slugify;

// Used for gitlab API access via HTTPS
#[macro_use]
extern crate hyper;
extern crate hyper_native_tls;
use hyper::client::Client;
use hyper::header::Headers;
use hyper::status::StatusCode;
use hyper::net::HttpsConnector;
use hyper_native_tls::NativeTlsClient;

// Custom header used to access the gitlab API
// See: https://docs.gitlab.com/ce/api/#authentication
header! { (PrivateToken, "PRIVATE-TOKEN") => [String] }

// Used to serialize JSON and YAML responses from the API
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;
#[macro_use]
extern crate serde_derive;

// Used to allow multiple paralell sync tasks
extern crate threadpool;
use threadpool::ThreadPool;
use std::sync::mpsc::channel;

// Used to do command line parsing
#[macro_use]
extern crate clap;
use clap::{Arg, App};

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
}

/// A project from the GitLab API
#[derive(Deserialize, Debug)]
struct Project {
    description: String,
    web_url: String,
    ssh_url_to_repo: String,
    http_url_to_repo: String,
}

/// A representation of a mirror job from orgin to destination
#[derive(Debug)]
struct Mirror {
    origin: String,
    destination: String,
}


fn get_mirror_repos(gitlab_url: &str,
                    group: &str,
                    headers: Headers,
                    use_http: bool)
                    -> Result<Vec<Mirror>, String> {
    let ssl = NativeTlsClient::new().expect("Unable to initialize TLS system");
    let connector = HttpsConnector::new(ssl);
    let client = Client::with_connector(connector);

    let url = format!("{}/api/v4/groups/{}/projects", gitlab_url, group);

    let res = client
        .get(&url)
        .headers(headers)
        .send()
        .or_else(|e| Err(format!("Unable to connect to: {} ({})", url, e)))?;

    if res.status != StatusCode::Ok {
        if res.status == StatusCode::Unauthorized {
            return Err(format!("API call received unautorized ({}) for: {}. \
                               Please make sure the `GITLAB_PRIVATE_TOKEN` environment \
                               variable is set.",
                               res.status,
                               url));
        } else {
            return Err(format!("API call received invalid status ({}) for : {}",
                               res.status,
                               url));
        }
    }

    let projects: Vec<Project> =
        serde_json::from_reader(res)
            .or_else(|e| Err(format!("Unable to parse response as JSON ({})", e)))?;

    let mut mirrors: Vec<Mirror> = Vec::new();

    for p in projects {
        match serde_yaml::from_str::<Desc>(&p.description) {
            Ok(desc) => {
                if desc.skip {
                    warn!("Skipping {}, Skip flag set", p.web_url);
                    continue;
                }
                println!("{0} -> {1}", desc.origin, p.ssh_url_to_repo);
                let destination = if use_http {
                    p.http_url_to_repo
                } else {
                    p.ssh_url_to_repo
                };
                let m = Mirror {
                    origin: desc.origin,
                    destination: destination,
                };
                mirrors.push(m);
            }
            Err(e) => warn!("Skipping {}, Description not valid YAML ({})", p.web_url, e),
        }
    }

    return Ok(mirrors);
}

fn mirror_repo(mirror_dir: String, origin: &str, destination: &str) -> Result<u8, String> {
    let origin_dir = Path::new(&mirror_dir).join(slugify(origin));

    debug!("Using origin dir: {0:?}", origin_dir);

    fs::DirBuilder::new()
        .recursive(true)
        .create(&mirror_dir)
        .or_else(|e| Err(format!("Unable to create mirror dir: {:?} ({})", mirror_dir, e)))?;

    if origin_dir.is_dir() {
        info!("Local Update for {}", origin);

        let mut set_url_cmd = Command::new("git");
        set_url_cmd
            .args(&["remote", "set-url", "origin"])
            .arg(origin)
            .current_dir(&origin_dir);

        trace!("Set url command started: {:?}", set_url_cmd);

        let out = set_url_cmd
            .output()
            .or_else(|e| {
                         Err(format!("Unable to execute set url command: {:?} ({})",
                                     set_url_cmd,
                                     e))
                     })?;

        debug!("Set url command: {0:?}, Status: {1}, Output: {2}",
               set_url_cmd,
               out.status,
               String::from_utf8_lossy(&out.stdout));

        if !out.status.success() {
            return Err(format!("Set url command ({:?}) failed with exit code: {}",
                               set_url_cmd,
                               out.status));
        }

        let mut remote_update_cmd = Command::new("git");
        remote_update_cmd
            .args(&["remote", "update"])
            .current_dir(&origin_dir);

        trace!("Remote update command started: {:?}", remote_update_cmd);

        let out2 = remote_update_cmd
            .output()
            .or_else(|e| {
                         Err(format!("Unable to execute remote update command: {:?} ({})",
                                     remote_update_cmd,
                                     e))
                     })?;

        debug!("Remote update command: {0:?}, Status: {1}, Output: {2}",
               remote_update_cmd,
               out2.status,
               String::from_utf8_lossy(&out2.stdout));

        if !out.status.success() {
            return Err(format!("Remote update command ({:?}) failed with exit code: {}",
                               remote_update_cmd,
                               out2.status));
        }

    } else if !origin_dir.exists() {
        info!("Local Checkout for {}", origin);

        let mut clone_cmd = Command::new("git");
        clone_cmd
            .args(&["clone", "--mirror"])
            .arg(origin)
            .arg(&origin_dir);

        trace!("Clone command started: {:?}", clone_cmd);

        let out =
            clone_cmd
                .output()
                .or_else(|e| {
                             Err(format!("Unable to execute clone command: {:?} ({})",
                                         clone_cmd,
                                         e))
                         })?;

        debug!("Clone command: {0:?}, Status: {1}, Output: {2}",
               clone_cmd,
               out.status,
               String::from_utf8_lossy(&out.stdout));

        if !out.status.success() {
            return Err(format!("Clone command ({:?}) failed with exit code: {}",
                               clone_cmd,
                               out.status));
        }

    } else {
        return Err(format!("Local origin dir is a file: {:?}", origin_dir));
    }

    info!("Push to destination {}", destination);

    let mut push_cmd = Command::new("git");
    push_cmd
        .args(&["push", "--mirror"])
        .arg(destination)
        .current_dir(&origin_dir);

    trace!("Push  started: {:?}", push_cmd);

    let out =
        push_cmd
            .output()
            .or_else(|e| Err(format!("Unable to execute push command: {:?} ({})", push_cmd, e)))?;

    debug!("Push command: {0:?}, Status: {1}, Output: {2}",
           push_cmd,
           out.status,
           String::from_utf8_lossy(&out.stdout));

    if !out.status.success() {
        return Err(format!("Push command ({:?}) failed with exit code: {}",
                           push_cmd,
                           out.status));
    }

    return Ok(1);
}

fn main() {
    let m = App::new("GitLab Sync")
        .author(crate_authors!())
        .version(crate_version!())
        .arg(Arg::with_name("url")
                 .short("u")
                 .long("url")
                 .help("URL of the Gitlab instance")
                 .default_value("https://gitlab.com"))
        .arg(Arg::with_name("group")
                 .short("g")
                 .long("group")
                 .help("Name of the group to check for repositires to sync")
                 .takes_value(true)
                 .required(true))
        .arg(Arg::with_name("mirror-dir")
                 .short("m")
                 .long("mirror-dir")
                 .help("Directory where the local clones are stored")
                 .default_value("./mirror-dir"))
        .arg(Arg::with_name("v")
                 .short("v")
                 .multiple(true)
                 .help("Verbosity level"))
        .arg(Arg::with_name("http")
                 .long("https")
                 .help("Use http(s) instead of SSH to sync the GitLab repository"))
        .arg(Arg::with_name("worker-count")
                 .short("c")
                 .long("worker-count")
                 .help("Number of concurrent mirror jobs")
                 .default_value("1"))
        .after_help("ENVIRONMENT:\n    GITLAB_PRIVATE_TOKEN    \
                     Private token or Personal access token to access the GitLab API")
        .get_matches();

    stderrlog::new()
        .module(module_path!())
        .verbosity(cmp::min(m.occurrences_of("v") as usize, 4))
        .init()
        .unwrap();

    let mut headers = Headers::new();
    match env::var("GITLAB_PRIVATE_TOKEN") {
        Ok(token) => {
            headers.set(PrivateToken(token));
        }
        Err(_) => trace!("GITLAB_PRIVATE_TOKEN not set"),
    }

    // Make sense of the arguments
    let mirror_dir = m.value_of("mirror-dir").unwrap();
    debug!("Using mirror directory: {}", mirror_dir);
    let gitlab_url = m.value_of("url").unwrap();
    debug!("Using gitlab url: {}", gitlab_url);
    let mirror_group = m.value_of("group").unwrap();
    debug!("Using group: {}", mirror_group);
    let use_http = m.is_present("http");
    debug!("Using http enabled: {}", use_http);
    let worker_count = match m.value_of("worker-count").unwrap().parse::<usize>() {
        Ok(count) => {
            if count > 0 {
                count
            } else {
                error!("Worker count can't be <= 0");
                exit(2);
            }
        }
        Err(e) => {
            error!("Worker count must be an integer > 0 ({})", e);
            exit(2);
        }
    };
    debug!("Worker count: {}", worker_count);

    // Get the list of repos to sync from gitlabsss
    let v = get_mirror_repos(&gitlab_url, &mirror_group, headers, use_http).unwrap_or_else(|e| {
        error!("Unable to get mirror repos ({})", e);
        exit(1);
    });

    // Give the work to the worker pool
    let pool = ThreadPool::new(worker_count);
    let mut n = 0;

    let (tx, rx) = channel();
    for x in v {
        let tx = tx.clone();
        let mirror_dir = mirror_dir.to_owned().clone();
        pool.execute(move || {

            let c = match mirror_repo(mirror_dir, &x.origin, &x.destination) {
                Ok(c) => c,
                Err(e) => {
                    error!("Unable to sync repo {} -> {} ({})",
                           x.origin,
                           x.destination,
                           e);
                    0
                }
            };
            tx.send(c).unwrap();
        });
        n += 1;
    }

    println!("Done {0}/{1}", rx.iter().take(n).fold(0, |a, b| a + b), n);

}
