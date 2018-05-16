/*
 * Copyright (c) 2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::path::PathBuf;
use std::process::Command;
use std::result;

type Result = result::Result<(), String>;

pub trait GitWrapper {
    fn git_version(&self) -> Result;
    fn git_clone_mirror(&self, origin: &str, repo_dir: &PathBuf) -> Result;
    fn git_update_mirror(&self, origin: &str, repo_dir: &PathBuf) -> Result;
    fn git_push_mirror(&self, dest: &str, repo_dir: &PathBuf) -> Result;
}

pub struct Git {
    executable: String,
}

impl Git {
    pub fn new(executable: String) -> Git {
        Git { executable }
    }

    fn git_base_cmd(&self) -> Command {
        let mut git = Command::new(self.executable.clone());
        git.env("GIT_TERMINAL_PROMPT", "0");
        git
    }

    fn run_cmd(&self, mut cmd: Command) -> Result {
        debug!("Run command: {:?}", cmd);
        match cmd.output() {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if !stdout.is_empty() {
                    debug!("Stdout: {}", stdout);
                }
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if !stderr.is_empty() {
                    debug!("Stderr: {}", stderr);
                }
                if o.status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "Command {:?} failed with exit code: {}, Stderr: {}",
                        cmd,
                        o.status.code().unwrap_or_default(),
                        stderr
                    ))
                }
            }
            Err(e) => Err(format!("Command {:?} failed with error: {}", cmd, e)),
        }
    }
}

impl GitWrapper for Git {
    fn git_version(&self) -> Result {
        let mut cmd = self.git_base_cmd();
        cmd.arg("--version");

        self.run_cmd(cmd)
    }

    fn git_clone_mirror(&self, origin: &str, repo_dir: &PathBuf) -> Result {
        let mut clone_cmd = self.git_base_cmd();
        clone_cmd
            .args(&["clone", "--mirror"])
            .arg(origin)
            .arg(repo_dir);

        self.run_cmd(clone_cmd)
    }

    fn git_update_mirror(&self, origin: &str, repo_dir: &PathBuf) -> Result {
        let mut set_url_cmd = self.git_base_cmd();
        set_url_cmd
            .current_dir(repo_dir)
            .args(&["remote", "set-url", "origin"])
            .arg(origin);

        self.run_cmd(set_url_cmd)?;

        let mut remote_update_cmd = self.git_base_cmd();
        remote_update_cmd
            .current_dir(&repo_dir)
            .args(&["remote", "update", "--prune"]);

        self.run_cmd(remote_update_cmd)
    }

    fn git_push_mirror(&self, dest: &str, repo_dir: &PathBuf) -> Result {
        let mut push_cmd = self.git_base_cmd();
        push_cmd
            .current_dir(repo_dir)
            .args(&["push", "--mirror"])
            .arg(dest);

        self.run_cmd(push_cmd)
    }
}
