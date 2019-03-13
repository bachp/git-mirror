/*
 * Copyright (c) 2017-2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

// Get Max of u32
use std::u32;

// Used for error and debug logging
use log::{debug, error, trace, warn};

use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode};

use crate::provider::{Mirror, MirrorError, MirrorResult, Provider};

#[derive(Debug)]
pub struct GitLab {
    pub url: String,
    pub group: String,
    pub use_http: bool,
    pub private_token: Option<String>,
    pub recursive: bool,
}

/// A structured description
#[derive(Deserialize, Debug)]
struct Desc {
    origin: String,
    #[serde(default)]
    skip: bool,
    #[serde(default)]
    flat: bool,
}

/// A project from the GitLab API
#[derive(Deserialize, Debug, Clone)]
struct Project {
    description: String,
    web_url: String,
    ssh_url_to_repo: String,
    http_url_to_repo: String,
}

/// A (sub)group from the GitLab API
#[derive(Deserialize, Debug, Clone)]
struct Group {
    id: u64,
}

// Number of items per page to request
const PER_PAGE: u8 = 100;

impl GitLab {
    fn get_paged<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        client: &Client,
        headers: &HeaderMap,
    ) -> Result<Vec<T>, String> {
        let mut results: Vec<T> = Vec::new();

        for page in 1..u32::MAX {
            let url = format!("{}?per_page={}&page={}", url, PER_PAGE, page);
            trace!("URL: {}", url);

            let res = client
                .get(&url)
                .headers(headers.clone())
                .send()
                .or_else(|e| Err(format!("Unable to connect to: {} ({})", url, e)))?;

            debug!("HTTP Status Received: {}", res.status());

            if res.status() != StatusCode::OK {
                if res.status() == StatusCode::UNAUTHORIZED {
                    return Err(format!(
                        "API call received unautorized ({}) for: {}. \
                         Please make sure the `GITLAB_PRIVATE_TOKEN` environment \
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

            let has_next = match res.headers().get("x-next-page") {
                None => {
                    trace!("No more pages, x-next-page header missing.");
                    false
                }
                Some(n) => {
                    if n.is_empty() {
                        trace!("No more pages, x-next-page-header empty.");
                        false
                    } else {
                        trace!("Next page: {:?}", n);
                        true
                    }
                }
            };

            let results_page: Vec<T> = serde_json::from_reader(res)
                .or_else(|e| Err(format!("Unable to parse response as JSON ({})", e)))?;

            results.extend(results_page);

            if !has_next {
                break;
            }
        }
        Ok(results)
    }

    fn get_projects(
        &self,
        id: &str,
        client: &Client,
        headers: &HeaderMap,
    ) -> Result<Vec<Project>, String> {
        let url = format!("{}/api/v4/groups/{}/projects", self.url, id);

        self.get_paged::<Project>(&url, client, headers)
    }

    fn get_subgroups(
        &self,
        id: &str,
        client: &Client,
        headers: &HeaderMap,
    ) -> Result<Vec<String>, String> {
        let url = format!("{}/api/v4/groups/{}/subgroups", self.url, id);

        let groups = self.get_paged::<Group>(&url, client, headers)?;

        let mut subgroups: Vec<String> = vec![id.to_owned()];

        for group in groups {
            subgroups.extend(self.get_subgroups(&format!("{}", group.id), client, headers)?);
        }

        Ok(subgroups)
    }
}

impl Provider for GitLab {
    fn get_label(&self) -> String {
        format!("{}/{}", self.url, self.group)
    }

    fn get_mirror_repos(&self) -> Result<Vec<MirrorResult>, String> {
        let client = Client::new();

        let use_http = self.use_http;

        let mut headers = HeaderMap::new();
        if let Some(ref token) = self.private_token {
            match HeaderValue::from_str(&token) {
                Ok(token) => {
                    headers.insert("PRIVATE-TOKEN", token);
                }
                Err(err) => {
                    error!("Unable to parse PRIVATE_TOKEN: {}", err);
                }
            }
        } else {
            warn!("PRIVATE_TOKEN not set")
        }

        let groups = if self.recursive {
            self.get_subgroups(&self.group, &client, &headers).or_else(
                |e| -> Result<Vec<String>, String> {
                    warn!("Unable to get subgroups: {}", e);
                    Ok(vec![self.group.clone()])
                },
            )?
        } else {
            vec![self.group.clone()]
        };

        let mut projects: Vec<Project> = Vec::new();

        for group in groups {
            projects.extend(self.get_projects(&group, &client, &headers)?);
        }

        let mut mirrors: Vec<MirrorResult> = Vec::new();

        for p in projects {
            let mut flat = false;
            match serde_yaml::from_str::<Desc>(&p.description) {
                Ok(desc) => {
                    if desc.skip {
                        mirrors.push(Err(MirrorError::Skip(p.web_url)));
                        continue;
                    }
                    if desc.flat {
                        flat = true;
                    }
                    trace!("{0} -> {1}", desc.origin, p.ssh_url_to_repo);
                    let destination = if use_http {
                        p.http_url_to_repo
                    } else {
                        p.ssh_url_to_repo
                    };
                    let m = Mirror {
                        origin: desc.origin,
                        destination,
                        flat: flat,
                    };
                    mirrors.push(Ok(m));
                }
                Err(e) => {
                    mirrors.push(Err(MirrorError::Description(p.web_url, e)));
                }
            }
        }

        Ok(mirrors)
    }
}
