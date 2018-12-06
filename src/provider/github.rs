/*
 * Copyright (c) 2017-2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

// Used for error and debug logging
use log::trace;

// Used for github API access via HTTPS
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::{Client, StatusCode};

use crate::provider::{Mirror, MirrorError, MirrorResult, Provider};

pub struct GitHub {
    pub url: String,
    pub org: String,
    pub use_http: bool,
    pub private_token: Option<String>,
    pub useragent: String,
}

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
}

/// A project from the GitLab API
#[derive(Deserialize, Debug)]
struct Project {
    description: Option<String>,
    url: String,
    ssh_url: String,
    clone_url: String,
}

impl Provider for GitHub {
    fn get_label(&self) -> String {
        format!("{}/orgs/{}", self.url, self.org)
    }

    fn get_mirror_repos(&self) -> Result<Vec<MirrorResult>, String> {
        let client = Client::new();

        let use_http = self.use_http;

        let mut headers = HeaderMap::new();
        // Github rejects requests without user agent
        let useragent = HeaderValue::from_str(&self.useragent).expect("User agent invalid!");
        headers.insert(USER_AGENT, useragent);
        // Set the accept header to make sure the v3 api is used
        let accept = HeaderValue::from_static("application/vnd.github.v3+json");
        headers.insert(ACCEPT, accept);

        let url = format!("{}/orgs/{}/repos", self.url, self.org);
        trace!("URL: {}", url);

        let res = client
            .get(&url)
            .headers(headers)
            .send()
            .or_else(|e| Err(format!("Unable to connect to: {} ({})", url, e)))?;

        if res.status() != StatusCode::OK {
            if res.status() == StatusCode::UNAUTHORIZED {
                return Err(format!(
                    "API call received unautorized ({}) for: {}. \
                     Please make sure the `GITHUB_PRIVATE_TOKEN` environment \
                     variable is set.",
                    res.status(),
                    url
                ));
            } else {
                return Err(format!(
                    "API call received invalid status ({}) for : {}",
                    res.status(),
                    url
                ));
            }
        }

        let projects: Vec<Project> = serde_json::from_reader(res)
            .or_else(|e| Err(format!("Unable to parse response as JSON ({:?})", e)))?;

        let mut mirrors: Vec<MirrorResult> = Vec::new();

        for p in projects {
            match serde_yaml::from_str::<Desc>(&p.description.unwrap_or_default()) {
                Ok(desc) => {
                    if desc.skip {
                        mirrors.push(Err(MirrorError::Skip(p.url)));
                        continue;
                    }
                    trace!("{0} -> {1}", desc.origin, p.ssh_url);
                    let destination = if use_http { p.clone_url } else { p.ssh_url };
                    let m = Mirror {
                        origin: desc.origin,
                        destination,
                    };
                    mirrors.push(Ok(m));
                }
                Err(e) => {
                    mirrors.push(Err(MirrorError::Description(p.url, e)));
                }
            }
        }
        Ok(mirrors)
    }
}
