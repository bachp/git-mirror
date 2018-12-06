/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

/// A representation of a mirror job from orgin to destination
#[derive(Debug)]
pub struct Mirror {
    pub origin: String,
    pub destination: String,
}

/// An error occuring during mirror creation
#[derive(Debug)]
pub enum MirrorError {
    Description(String, serde_yaml::Error),
    Skip(String),
}

pub type MirrorResult = Result<Mirror, MirrorError>;

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
}

pub trait Provider {
    fn get_mirror_repos(&self) -> Result<Vec<MirrorResult>, String>;
    fn get_label(&self) -> String;
}

mod gitlab;
pub use self::gitlab::GitLab;

mod github;
pub use self::github::GitHub;
