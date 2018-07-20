/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

// Used for error and debug logging
extern crate log;

// Used for github API access via HTTPS
use reqwest::header::{qitem, Accept, Headers, UserAgent};
use reqwest::{Client, StatusCode};

// Used to serialize JSON and YAML responses from the API
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;

use provider::{Mirror, MirrorError, MirrorResult, Provider};

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

        let mut headers = Headers::new();
        // Github rejects requests without user agent
        headers.set(UserAgent::new(self.useragent.to_owned()));
        // Set the accept header to make sure the v3 api is used
        headers.set(Accept(vec![qitem(
            "application/vnd.github.v3+json".parse().unwrap(),
        )]));

        let url = format!("{}/orgs/{}/repos", self.url, self.org);
        trace!("URL: {}", url);

        let res = client
            .get(&url)
            .headers(headers)
            .send()
            .or_else(|e| Err(format!("Unable to connect to: {} ({})", url, e)))?;

        if res.status() != StatusCode::Ok {
            if res.status() == StatusCode::Unauthorized {
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
                    let destination = if use_http {
                        p.clone_url
                    } else {
                        p.ssh_url
                    };
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
