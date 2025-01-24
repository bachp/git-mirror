/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::cmp;
use std::env;

// Used for error and debug logging
use env_logger::Env;
use log::{debug, error, info};

// Used to do command line parsing
use clap::{crate_name, crate_version};
use clap::{ArgAction, Parser, ValueEnum};
use std::path::PathBuf;

// Load the real functionality
use git_mirror::do_mirror;
use git_mirror::provider::{GitHub, GitLab, Provider};
use git_mirror::MirrorOptions;

use std::process::exit;

#[derive(ValueEnum, Clone, Debug)]
#[value(rename_all = "verbatim")]
enum Providers {
    GitLab,
    GitHub,
}

/// command line options
#[derive(Parser, Debug)]
#[command(name = "git-mirror", version, about)]
struct Opt {
    /// Provider to use for fetching repositories
    #[arg(
        long = "provider",
        short = 'p',
        default_value = "GitLab",
        ignore_case = true,
        value_enum
    )]
    provider: Providers,

    /// URL of the instance to get repositories from
    #[arg(
        long = "url",
        short = 'u',
        default_value_ifs([
            ("provider", "GitLab", Some("https://gitlab.com")),
            ("provider", "GitHub", Some("https://api.github.com")),
        ])
    )]
    url: String,

    /// Name of the group to check for repositories to sync
    #[arg(long = "group", short = 'g')]
    group: String,

    /// Directory where the local clones are stored
    #[arg(long = "mirror-dir", short = 'm', default_value = "./mirror-dir")]
    mirror_dir: PathBuf,

    /// Verbosity level
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Use http(s) instead of SSH to sync the GitLab repository
    #[arg(long)]
    http: bool,

    /// Only print what to do without actually running any git commands
    #[arg(long)]
    dry_run: bool,

    /// Number of concurrent mirror jobs
    #[arg(short = 'c', long, default_value = "1")]
    worker_count: usize,

    /// Location where to store metrics for consumption by
    /// Prometheus node exporter's text file colloctor
    #[arg(long)]
    metric_file: Option<PathBuf>,

    /// Location where to store the Junit XML report
    #[arg(long)]
    junit_report: Option<PathBuf>,

    /// Git executable to use
    #[arg(long, default_value = "git")]
    git_executable: String,

    /// Private token or Personal access token to access the GitLab or GitHub API
    #[arg(long, env = "PRIVATE_TOKEN")]
    private_token: Option<String>,

    /// Default refspec used to mirror repositories, can be overridden per project
    #[arg(long)]
    refspec: Option<Vec<String>>,

    /// Remove the local working repository after pushing. This requires a full re-clone on the next run.
    #[arg(long)]
    remove_workrepo: bool,

    /// Fail on sync task error. If set the executable will exit with 1 if any sync task failed.
    #[arg(long)]
    fail_on_sync_error: bool,

    /// Mirror lfs objects as well
    #[arg(long, default_value = "false")]
    lfs: bool,
}

impl From<Opt> for MirrorOptions {
    fn from(opt: Opt) -> MirrorOptions {
        MirrorOptions {
            mirror_dir: opt.mirror_dir,
            dry_run: opt.dry_run,
            worker_count: opt.worker_count,
            metrics_file: opt.metric_file,
            junit_file: opt.junit_report,
            git_executable: opt.git_executable,
            refspec: opt.refspec,
            remove_workrepo: opt.remove_workrepo,
            fail_on_sync_error: opt.fail_on_sync_error,
            mirror_lfs: opt.lfs,
        }
    }
}

fn main() {
    // Setup commandline parser
    let opt = Opt::parse();
    debug!("{:#?}", opt);

    let env_log_level = match cmp::min(opt.verbose, 4) {
        4 => "git_mirror=trace",
        3 => "git_mirror=debug",
        2 => "git_mirror=info",
        1 => "git_mirror=warn",
        _ => "git_mirror=error",
    };
    env_logger::Builder::from_env(Env::default().default_filter_or(env_log_level)).init();

    // Run OpenSSL probing on all platforms even the ones not using it
    // TODO: Switch to rustls-platform-verifier and rustls once it is supported by
    // reqwest https://github.com/seanmonstar/reqwest/issues/2159
    unsafe {
        openssl_probe::init_openssl_env_vars();
    };

    let provider: Box<dyn Provider> = match opt.provider {
        Providers::GitLab => Box::new(GitLab {
            url: opt.url.to_owned(),
            group: opt.group.to_owned(),
            use_http: opt.http,
            private_token: opt.private_token.to_owned(),
            recursive: true,
        }),
        Providers::GitHub => Box::new(GitHub {
            url: opt.url.to_owned(),
            org: opt.group.to_owned(),
            use_http: opt.http,
            private_token: opt.private_token.to_owned(),
            useragent: format!("{}/{}", crate_name!(), crate_version!()),
        }),
    };

    let opts: MirrorOptions = opt.into();

    match do_mirror(provider, &opts) {
        Ok(_) => {
            info!("All done");
        }
        Err(e) => {
            error!("Error occured: {}", e);
            exit(e.into());
        }
    };
}

#[cfg(test)]
mod tests {
    use super::Opt;

    #[test]
    fn verify_app() {
        use clap::CommandFactory;
        Opt::command().debug_assert()
    }
}
