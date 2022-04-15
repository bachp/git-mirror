use crate::git::GitError;
use crate::provider::MirrorError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitMirrorError {
    #[error("Generic Mirror error: {0}")]
    GenericError(String),
    #[error("Git command execution failed: {0}")]
    GitError(#[from] GitError),
    #[error("Mirror extraction failed: {0}")]
    MirrorError(#[from] MirrorError),
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
