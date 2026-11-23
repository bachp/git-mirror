/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use thiserror::Error;

use crate::Shard;
/// A representation of a mirror job from origin to destination
#[derive(Debug)]
pub struct Mirror {
    pub origin: String,
    pub destination: String,
    pub refspec: Option<Vec<String>>,
    pub lfs: bool,
}

/// An error occurring during mirror creation
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

    /// Discover mirror jobs, optionally restricting them to a shard.
    fn get_mirror_repos_for_shard(
        &self,
        shard: Option<Shard>,
    ) -> Result<Vec<MirrorResult>, String> {
        if shard.is_some() {
            return Err("Provider does not support sharding".to_owned());
        }
        self.get_mirror_repos()
    }
}

mod gitlab;
pub use self::gitlab::GitLab;

mod github;
pub use self::github::GitHub;
