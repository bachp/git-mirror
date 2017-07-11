/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::env;
use std::cmp;

// Used for error and debug logging
#[macro_use]
extern crate log;
extern crate stderrlog;


// Used to do command line parsing
#[macro_use]
extern crate clap;
use clap::{Arg, App};

// Load the real functionality
extern crate git_mirror;
use git_mirror::do_mirror;
use git_mirror::provider::{GitLab, GitHub};

arg_enum!{
    #[derive(Debug)]
    enum Providers {
      GitLab,
      GitHub
    }
}

fn main() {
    let m = App::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("url")
                .short("u")
                .long("url")
                .help("URL of the instance to get repositires from")
                .default_value_if("provider", Some("GitLab"), "https://gitlab.com")
                .default_value_if("provider", Some("GitHub"), "https://api.github.com"),
        )
        .arg(
            Arg::with_name("group")
                .short("g")
                .long("group")
                .help("Name of the group to check for repositires to sync")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("mirror-dir")
                .short("m")
                .long("mirror-dir")
                .help("Directory where the local clones are stored")
                .default_value("./mirror-dir"),
        )
        .arg(Arg::with_name("v").short("v").multiple(true).help(
            "Verbosity level",
        ))
        .arg(Arg::with_name("http").long("https").help(
            "Use http(s) instead of SSH to sync the GitLab repository",
        ))
        .arg(Arg::with_name("dry-run").long("dry-run").help(
            "Only print what to do without actually running any git commands.",
        ))
        .arg(
            Arg::with_name("worker-count")
                .short("c")
                .long("worker-count")
                .help("Number of concurrent mirror jobs")
                .default_value("1"),
        )
        .arg(
            Arg::with_name("provider")
                .short("p")
                .long("provider")
                .help("Provider to use for fetching repositories")
                .takes_value(true)
                .possible_values(&Providers::variants())
                .default_value("GitLab"),
        )
        .after_help(
            "ENVIRONMENT:\n    GITLAB_PRIVATE_TOKEN    \
                     Private token or Personal access token to access the GitLab API",
        )
        .get_matches();

    stderrlog::new()
        .module(module_path!())
        .verbosity(cmp::min(m.occurrences_of("v") as usize, 4))
        .init()
        .unwrap();

    let gitlab_private_token = env::var("GITLAB_PRIVATE_TOKEN").ok();

    // Make sense of the arguments
    let mirror_dir = value_t_or_exit!(m.value_of("mirror-dir"), String);
    debug!("Using mirror directory: {}", mirror_dir);
    let gitlab_url = value_t_or_exit!(m.value_of("url"), String);
    debug!("Using gitlab url: {}", gitlab_url);
    let mirror_group = value_t_or_exit!(m.value_of("group"), String);
    debug!("Using group: {}", mirror_group);
    let use_http = m.is_present("http");
    debug!("Using http enabled: {}", use_http);
    let dry_run = m.is_present("dry-run");
    debug!("Dry run: {}", dry_run);
    let worker_count = value_t_or_exit!(m.value_of("worker-count"), usize);
    debug!("Worker count: {}", worker_count);

    let provider = value_t_or_exit!(m.value_of("provider"), Providers);

    match provider {
        Providers::GitLab => {
            let p = GitLab {
                url: gitlab_url.to_owned(),
                group: mirror_group.to_owned(),
                use_http: use_http,
                private_token: gitlab_private_token,
            };
            do_mirror(&p, worker_count, &mirror_dir, dry_run);
        }
        Providers::GitHub => {
            let p = GitHub {
                url: gitlab_url.to_owned(),
                org: mirror_group.to_owned(),
                use_http: use_http,
                private_token: gitlab_private_token,
                useragent: format!("{}/{}", crate_name!(), crate_version!()),
            };
            do_mirror(&p, worker_count, &mirror_dir, dry_run);
        }
    };
}
