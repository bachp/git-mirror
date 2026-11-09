/*
 * Copyright (c) 2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use command_group::CommandGroup;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

use log::debug;
use wait_timeout::ChildExt;

/// An error occurring during git command execution
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
        let start = Instant::now();
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .group_spawn()?;
        let stdout = child.inner().stdout.take().unwrap();
        let stderr = child.inner().stderr.take().unwrap();
        drop(child.inner().stdin.take());

        thread::scope(|scope| {
            let (stdout_tx, stdout_rx) = mpsc::channel();
            let (stderr_tx, stderr_rx) = mpsc::channel();
            scope.spawn(move || {
                let _ = stdout_tx.send(read_output(stdout));
            });
            scope.spawn(move || {
                let _ = stderr_tx.send(read_output(stderr));
            });

            let remaining = || timeout.saturating_sub(start.elapsed());
            let collect = |receiver: Receiver<io::Result<Vec<u8>>>| match receiver
                .recv_timeout(remaining())
            {
                Ok(output) => output.map_err(CommandExecutionError::from),
                Err(RecvTimeoutError::Timeout) => {
                    Err(CommandExecutionError::TimeoutReachedError(timeout))
                }
                Err(error) => Err(io::Error::other(error).into()),
            };

            let result = (|| {
                let stdout = collect(stdout_rx)?;
                let stderr = collect(stderr_rx)?;
                let status = child
                    .inner()
                    .wait_timeout(remaining())?
                    .ok_or(CommandExecutionError::TimeoutReachedError(timeout))?;
                Ok(Output {
                    status,
                    stdout,
                    stderr,
                })
            })();

            if result.is_err() {
                if let Err(error) = child.kill() {
                    debug!("Unable to terminate Git process group: {error}");
                }
                child.wait()?;
            }

            result
        })
    }
}

fn read_output(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader.read_to_end(&mut output)?;
    Ok(output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Instant;

    const OUTPUT_SIZE: usize = 2 * 1024 * 1024;

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "git::tests::command_fixture",
                "--nocapture",
            ])
            .env("GIT_MIRROR_TEST_COMMAND", mode);
        command
    }

    #[test]
    #[ignore = "subprocess fixture"]
    fn command_fixture() {
        let mode = std::env::var("GIT_MIRROR_TEST_COMMAND").unwrap();
        match mode.as_str() {
            "stdout" | "stderr" | "both" => {
                let mut stdout = io::stdout().lock();
                let mut stderr = io::stderr().lock();
                let chunk = [0; 8192];
                for _ in 0..OUTPUT_SIZE / chunk.len() {
                    if mode != "stderr" {
                        stdout.write_all(&chunk).unwrap();
                    }
                    if mode != "stdout" {
                        stderr.write_all(&chunk).unwrap();
                    }
                }
            }
            "failure" => {
                eprintln!("fixture failed");
                std::process::exit(7);
            }
            "sleep" => std::thread::sleep(Duration::from_secs(30)),
            "descendant" => {
                fixture("sleep").spawn().unwrap().wait().unwrap();
            }
            "orphan" => {
                #[allow(
                    clippy::zombie_processes,
                    reason = "The fixture must exit while its child still holds the output pipes"
                )]
                let _child = fixture("sleep").spawn().unwrap();
            }
            _ => panic!("Unknown fixture mode: {mode}"),
        }
    }

    fn assert_large_output(mode: &str) {
        let git = Git::new("git".into(), false, None);
        let output = git
            .run_cmd_with_timeout(fixture(mode), Duration::from_secs(5))
            .unwrap();

        assert!(output.status.success());
        for (stream, expected) in [
            (&output.stdout, mode != "stderr"),
            (&output.stderr, mode != "stdout"),
        ] {
            assert_eq!(
                stream.iter().filter(|&&byte| byte == 0).count(),
                if expected { OUTPUT_SIZE } else { 0 }
            );
        }
    }

    #[test]
    fn captures_large_stdout_with_timeout() {
        assert_large_output("stdout");
    }

    #[test]
    fn captures_large_stderr_with_timeout() {
        assert_large_output("stderr");
    }

    #[test]
    fn captures_both_large_streams_with_timeout() {
        assert_large_output("both");
    }

    #[test]
    fn preserves_exit_code_and_stderr_with_timeout() {
        let git = Git::new("git".into(), false, Some(Duration::from_secs(5)));
        let error = git.run_cmd(fixture("failure")).unwrap_err();

        assert!(matches!(
            *error,
            GitError::GitCommandError { code: 7, ref stderr, .. }
                if stderr.contains("fixture failed")
        ));
    }

    fn assert_timeout(mode: &str) {
        let timeout = Duration::from_millis(500);
        let git = Git::new("git".into(), false, Some(timeout));
        let start = Instant::now();
        let error = git.run_cmd(fixture(mode)).unwrap_err();

        assert!(matches!(
            *error,
            GitError::GitCommandTimeout { timeout: actual, .. } if actual == timeout
        ));
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn terminates_command_on_timeout() {
        assert_timeout("sleep");
    }

    #[test]
    fn terminates_descendants_on_timeout() {
        assert_timeout("descendant");
    }

    #[test]
    fn times_out_when_exited_command_leaves_output_pipes_open() {
        assert_timeout("orphan");
    }
}
