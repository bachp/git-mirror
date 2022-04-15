/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use thiserror::Error;
/// A representation of a mirror job from orgin to destination
#[derive(Debug)]
pub struct Mirror {
    pub origin: String,
    pub destination: String,
    pub refspec: Option<Vec<String>>,
}

/// An error occuring during mirror creation
#[derive(Debug, Error)]
pub enum MirrorError {
    #[error("data store disconnected")]
    Description(String, serde_yaml::Error),
    #[error("entry explicitly skiped")]
    Skip(String),
}

pub type MirrorResult = Result<Mirror, MirrorError>;

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
    refspec: Option<Vec<String>>,
}

pub trait Provider {
    fn get_mirror_repos(&self) -> Result<Vec<MirrorResult>, String>;
    fn get_label(&self) -> String;
}

mod gitlab;
pub use self::gitlab::GitLab;

mod github;
pub use self::github::GitHub;
