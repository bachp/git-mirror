use crate::git::GitError;
use crate::provider::MirrorError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitMirrorError {
    #[error("Generic Mirror error: {0}")]
    GenericError(String),
    #[error("Git command execution failed: {0}")]
    GitError(#[from] Box<GitError>),
    #[error("Mirror extraction failed: {0}")]
    MirrorError(#[from] Box<MirrorError>),
    #[error("{0} sync tasks failed")]
    SyncError(usize),
}

impl From<GitMirrorError> for i32 {
    fn from(mirror: GitMirrorError) -> i32 {
        match mirror {
            GitMirrorError::SyncError(_) => 1,
            GitMirrorError::GenericError(_) => 2,
            GitMirrorError::GitError(_) => 3,
            GitMirrorError::MirrorError(_) => 4,
        }
    }
}

pub type Result<T> = core::result::Result<T, GitMirrorError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::CommandExecutionError;

    #[test]
    fn test_git_mirror_error_display() {
        let err = GitMirrorError::GenericError("test error".to_string());
        assert_eq!(format!("{}", err), "Generic Mirror error: test error");

        let git_err = GitError::CommandError {
            cmd_str: "git status".to_string(),
            err: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let err = GitMirrorError::GitError(Box::new(git_err));
        assert!(format!("{}", err).contains("Git command execution failed"));

        let err = GitMirrorError::SyncError(5);
        assert_eq!(format!("{}", err), "5 sync tasks failed");
    }

    #[test]
    fn test_git_mirror_error_into_i32() {
        assert_eq!(i32::from(GitMirrorError::SyncError(5)), 1);
        assert_eq!(
            i32::from(GitMirrorError::GenericError("x".to_string())),
            2
        );
        let git_err = GitError::CommandError {
            cmd_str: "git".to_string(),
            err: std::io::Error::new(std::io::ErrorKind::Other, "err"),
        };
        assert_eq!(i32::from(GitMirrorError::GitError(Box::new(git_err))), 3);
        let mirror_err = MirrorError::Description("url".to_string(), serde_yaml::from_str::<serde_yaml::Value>("{{").unwrap_err());
        assert_eq!(i32::from(GitMirrorError::MirrorError(Box::new(mirror_err))), 4);
    }

    #[test]
    fn test_git_error_display() {
        let err = GitError::CommandError {
            cmd_str: "git status".to_string(),
            err: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        assert!(format!("{}", err).contains("Command git status failed with system error"));

        let err = GitError::GitCommandError {
            code: 1,
            stderr: "fatal".to_string(),
            cmd_str: "git push".to_string(),
        };
        assert!(format!("{}", err).contains("Command git push failed with exit code: 1"));

        let err = GitError::GitCommandTimeout {
            cmd_str: "git clone".to_string(),
            timeout: std::time::Duration::from_secs(30),
        };
        assert!(format!("{}", err).contains("Command git clone timed out"));
    }

    #[test]
    fn test_command_execution_error_display() {
        let err = CommandExecutionError::SystemIOError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(format!("{}", err).contains("Unknown system IO error"));

        let err = CommandExecutionError::TimeoutReachedError(std::time::Duration::from_secs(10));
        assert!(format!("{}", err).contains("Timeout has been reached"));
    }

    #[test]
    fn test_from_command_execution_error_to_git_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let cmd_str = "git status".to_string();
        let git_err: GitError = (CommandExecutionError::SystemIOError(io_err), cmd_str.clone()).into();
        assert!(matches!(git_err, GitError::CommandError { .. }));
        if let GitError::CommandError { cmd_str: s, .. } = git_err {
            assert_eq!(s, cmd_str);
        }
    }

    #[test]
    fn test_from_timeout_to_git_error() {
        let dur = std::time::Duration::from_secs(30);
        let cmd_str = "git clone".to_string();
        let git_err: GitError = (CommandExecutionError::TimeoutReachedError(dur), cmd_str.clone()).into();
        assert!(matches!(git_err, GitError::GitCommandTimeout { .. }));
        if let GitError::GitCommandTimeout { cmd_str: s, .. } = git_err {
            assert_eq!(s, cmd_str);
        }
    }
}
