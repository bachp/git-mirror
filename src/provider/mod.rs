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

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
}

pub trait Provider {
    fn get_mirror_repos(&self) -> Result<Vec<Mirror>, String>;
}

mod gitlab;
pub use self::gitlab::GitLab;

mod github;
pub use self::github::GitHub;
