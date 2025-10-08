/*
 * Copyright (c) 2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use thiserror::Error;

use log::debug;
use wait_timeout::ChildExt;

/// An error occuring during git command execution
#[derive(Debug, Error)]
pub enum GitError {
    #[error("Command {cmd_str} failed with system error: {err}")]
    CommandError { cmd_str: String, err: io::Error },
    #[error("Command {cmd_str} failed with exit code: {code}, Stderr: {stderr}")]
    GitCommandError {
        code: i32,
        stderr: String,
        cmd_str: String,
    },
    #[error("Command {cmd_str} timed out after {timeout:?}")]
    GitCommandTimeout { cmd_str: String, timeout: Duration },
}

#[derive(Debug, Error)]
pub enum CommandExecutionError {
    #[error("Unknown system IO error: {0}")]
    SystemIOError(#[from] io::Error),
    #[error("Timeout has been reached: {0:?}")]
    TimeoutReachedError(Duration),
}

impl From<(CommandExecutionError, String)> for GitError {
    fn from(value: (CommandExecutionError, String)) -> Self {
        match value {
            (CommandExecutionError::SystemIOError(err), cmd_str) => {
                GitError::CommandError { cmd_str, err }
            }
            (CommandExecutionError::TimeoutReachedError(timeout), cmd_str) => {
                GitError::GitCommandTimeout { cmd_str, timeout }
            }
        }
    }
}

/// Common interface to different git backends
/// - [x] git command line
/// - [ ] libgit2
/// - [ ] gitoxide
///
pub trait GitWrapper {
    /// Get the git version
    fn git_version(&self) -> Result<(), Box<GitError>>;
    fn git_lfs_version(&self) -> Result<(), Box<GitError>>;
    fn git_clone_mirror(
        &self,
        origin: &str,
        repo_dir: &Path,
        lfs: bool,
    ) -> Result<(), Box<GitError>>;
    fn git_update_mirror(
        &self,
        origin: &str,
        repo_dir: &Path,
        lfs: bool,
    ) -> Result<(), Box<GitError>>;
    fn git_push_mirror(
        &self,
        dest: &str,
        repo_dir: &Path,
        refspec: &Option<Vec<String>>,
        lfs: bool,
    ) -> Result<(), Box<GitError>>;
}

/// Git command line wrapper
pub struct Git {
    executable: String,
    lfs_enabled: bool,
    timeout: Option<Duration>,
}

impl Git {
    pub fn new(executable: String, lfs_enabled: bool, timeout: Option<Duration>) -> Git {
        Git {
            executable,
            lfs_enabled,
            timeout,
        }
    }

    fn git_base_cmd(&self) -> Command {
        let mut git = Command::new(self.executable.clone());
        git.env("GIT_TERMINAL_PROMPT", "0");
        git
    }

    fn run_cmd(&self, mut cmd: Command) -> Result<(), Box<GitError>> {
        let cmd_str = format!("{:?}", cmd);

        let result: Result<Output, CommandExecutionError> = match self.timeout {
            Some(timeout) => self.run_cmd_with_timeout(cmd, timeout),
            None => cmd.output().map_err(From::from),
        };

        match result {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                if !stdout.is_empty() {
                    debug!("Stdout: {stdout}");
                }
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                if !stderr.is_empty() {
                    debug!("Stderr: {stderr}");
                }
                if o.status.success() {
                    Ok(())
                } else {
                    Err(Box::new(GitError::GitCommandError {
                        cmd_str,
                        code: o.status.code().unwrap_or_default(),
                        stderr,
                    }))
                }
            }
            Err(err) => Err(Box::new((err, cmd_str).into())),
        }
    }

    fn run_cmd_with_timeout(
        &self,
        mut cmd: Command,
        timeout: Duration,
    ) -> Result<Output, CommandExecutionError> {
        let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        match child.wait_timeout(timeout)? {
            Some(_) => Ok(child.wait_with_output()?),
            None => {
                child.kill()?;
                Err(CommandExecutionError::TimeoutReachedError(timeout))
            }
        }
    }
}

impl GitWrapper for Git {
    fn git_version(&self) -> Result<(), Box<GitError>> {
        let mut cmd = self.git_base_cmd();
        cmd.arg("--version");

        self.run_cmd(cmd)
    }

    fn git_lfs_version(&self) -> Result<(), Box<GitError>> {
        let mut cmd = self.git_base_cmd();
        cmd.arg("lfs");
        cmd.arg("version");

        self.run_cmd(cmd)
    }

    fn git_clone_mirror(
        &self,
        origin: &str,
        repo_dir: &Path,
        lfs: bool,
    ) -> Result<(), Box<GitError>> {
        let mut clone_cmd = self.git_base_cmd();
        clone_cmd
            .args(["clone", "--mirror"])
            .arg(origin)
            .arg(repo_dir);

        self.run_cmd(clone_cmd)?;

        if self.lfs_enabled && lfs {
            let mut lfs_fetch_cmd = self.git_base_cmd();
            lfs_fetch_cmd.args(["lfs", "fetch"]).current_dir(repo_dir);

            self.run_cmd(lfs_fetch_cmd)
        } else {
            Ok(())
        }
    }

    fn git_update_mirror(
        &self,
        origin: &str,
        repo_dir: &Path,
        lfs: bool,
    ) -> Result<(), Box<GitError>> {
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

        self.run_cmd(remote_update_cmd)?;

        if self.lfs_enabled && lfs {
            let mut lfs_fetch_cmd = self.git_base_cmd();
            lfs_fetch_cmd.args(["lfs", "fetch"]).current_dir(repo_dir);

            self.run_cmd(lfs_fetch_cmd)
        } else {
            Ok(())
        }
    }

    fn git_push_mirror(
        &self,
        dest: &str,
        repo_dir: &Path,
        refspec: &Option<Vec<String>>,
        lfs: bool,
    ) -> Result<(), Box<GitError>> {
        if self.lfs_enabled && lfs {
            let mut lfs_install_cmd = self.git_base_cmd();
            lfs_install_cmd
                .args(["lfs", "install"])
                .current_dir(repo_dir);
            self.run_cmd(lfs_install_cmd)?;
        }

        let mut push_cmd = self.git_base_cmd();
        push_cmd.current_dir(repo_dir);
        // override the git lfs url when pushing, in case a .lfsconfig with a different URL exists
        push_cmd.args(["-c", &format!("lfs.url={dest}")]);
        push_cmd.args(["push", "-f"]);
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
