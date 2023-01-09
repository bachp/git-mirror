/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use thiserror::Error;
/// A representation of a mirror job from origin to destination
#[derive(Debug)]
pub struct Mirror {
    pub origin: String,
    pub destination: String,
    pub refspec: Option<Vec<String>>,
    pub lfs: bool,
}

/// An error occuring during mirror creation
#[derive(Debug, Error)]
pub enum MirrorError {
    #[error("data store disconnected")]
    Description(String, serde_yaml::Error),
    #[error("entry explicitly skipped")]
    Skip(String),
}

#[inline]
pub fn bool_true() -> bool {
    true
}

pub type MirrorResult = Result<Mirror, MirrorError>;

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
    refspec: Option<Vec<String>>,
    #[serde(default = "bool_true")]
    lfs: bool,
}

pub trait Provider {
    fn get_mirror_repos(&self) -> Result<Vec<MirrorResult>, String>;
    fn get_label(&self) -> String;
}

mod gitlab;
pub use self::gitlab::GitLab;

mod github;
pub use self::github::GitHub;
