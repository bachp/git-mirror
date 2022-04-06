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
}

impl Into<i32> for GitMirrorError {
    fn into(self) -> i32 {
        match self {
            Self::GenericError(_) => 2,
            Self::GitError(_) => 3,
            Self::MirrorError(_) => 4,
        }
    }
}

pub type Result<T> = core::result::Result<T, GitMirrorError>;
