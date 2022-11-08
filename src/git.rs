/*
 * Copyright (c) 2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::path::Path;
use std::process::Command;
use thiserror::Error;

use log::debug;

/// An error occuring during git command execution
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Command {cmd:?} failed with error: {err}")]
    CommandError { cmd: Command, err: std::io::Error },
    #[error("Command {cmd:?} failed with exit code: {code}, Stderr: {stderr}")]
    GitCommandError {
        code: i32,
        stderr: String,
        cmd: Command,
    },
}

/// Common interface to different git backends
/// - [x] git command line
/// - [ ] libgit2
/// - [ ] gitoxide
///
pub trait GitWrapper {
    /// Get the git version
    fn git_version(&self) -> Result<(), GitError>;
    fn git_clone_mirror(&self, origin: &str, repo_dir: &Path) -> Result<(), GitError>;
    fn git_update_mirror(&self, origin: &str, repo_dir: &Path) -> Result<(), GitError>;
    fn git_push_mirror(
        &self,
        dest: &str,
        repo_dir: &Path,
        refspec: &Option<Vec<String>>,
    ) -> Result<(), GitError>;
}

/// Git command line wrapper
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

    fn run_cmd(&self, mut cmd: Command) -> Result<(), GitError> {
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
                    Err(GitError::GitCommandError {
                        cmd,
                        code: o.status.code().unwrap_or_default(),
                        stderr,
                    })
                }
            }
            Err(e) => Err(GitError::CommandError { cmd, err: e }),
        }
    }
}

impl GitWrapper for Git {
    fn git_version(&self) -> Result<(), GitError> {
        let mut cmd = self.git_base_cmd();
        cmd.arg("--version");

        self.run_cmd(cmd)
    }

    fn git_clone_mirror(&self, origin: &str, repo_dir: &Path) -> Result<(), GitError> {
        let mut clone_cmd = self.git_base_cmd();
        clone_cmd
            .args(["clone", "--mirror"])
            .arg(origin)
            .arg(repo_dir);

        self.run_cmd(clone_cmd)
    }

    fn git_update_mirror(&self, origin: &str, repo_dir: &Path) -> Result<(), GitError> {
        let mut set_url_cmd = self.git_base_cmd();
        set_url_cmd
            .current_dir(repo_dir)
            .args(["remote", "set-url", "origin"])
            .arg(origin);

        self.run_cmd(set_url_cmd)?;

        let mut remote_update_cmd = self.git_base_cmd();
        remote_update_cmd
            .current_dir(repo_dir)
            .args(["remote", "update", "--prune"]);

        self.run_cmd(remote_update_cmd)
    }

    fn git_push_mirror(
        &self,
        dest: &str,
        repo_dir: &Path,
        refspec: &Option<Vec<String>>,
    ) -> Result<(), GitError> {
        let mut push_cmd = self.git_base_cmd();
        push_cmd.current_dir(repo_dir);
        push_cmd.arg("push");
        if let Some(r) = &refspec {
            push_cmd.arg(dest);
            for spec in r.iter() {
                push_cmd.arg(spec);
            }
        } else {
            push_cmd.args(["--mirror", dest]);
        }
        self.run_cmd(push_cmd)
    }
}
