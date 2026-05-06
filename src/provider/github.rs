/*
 * Copyright (c) 2017-2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

// Used for error and debug logging
use log::trace;

// Used for github API access via HTTPS
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};

use crate::provider::{Desc, Mirror, MirrorError, MirrorResult, Provider};

pub struct GitHub {
    pub url: String,
    pub org: String,
    pub use_http: bool,
    pub private_token: Option<String>,
    pub useragent: String,
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
        trace!("URL: {url}");

        let res = client
            .get(&url)
            .headers(headers)
            .send()
            .map_err(|e| format!("Unable to connect to: {url} ({e})"))?;

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
            .map_err(|e| format!("Unable to parse response as JSON ({e:?})"))?;

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
                        refspec: desc.refspec,
                        lfs: desc.lfs,
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

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn create_github(server_url: String) -> GitHub {
        GitHub {
            url: server_url,
            org: "myorg".to_string(),
            use_http: false,
            private_token: None,
            useragent: "test".to_string(),
        }
    }

    #[test]
    fn test_github_get_label() {
        let github = create_github("https://api.github.com".to_string());
        assert_eq!(github.get_label(), "https://api.github.com/orgs/myorg");
    }

    #[test]
    fn test_get_mirror_repos_success() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git","url":"https://api.github.com/myorg/repo","ssh_url":"git@github.com:myorg/repo.git","clone_url":"https://github.com/myorg/repo.git"}]"#,
            )
            .create();

        let github = create_github(server.url());
        let result = github.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        let repo = result[0].as_ref().unwrap();
        assert_eq!(repo.origin, "https://github.com/test/repo.git");
        assert_eq!(repo.destination, "git@github.com:myorg/repo.git");
        assert!(repo.lfs);
    }

    #[test]
    fn test_get_mirror_repos_skip() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git\nskip: true\n","url":"https://api.github.com/myorg/repo","ssh_url":"git@github.com:myorg/repo.git","clone_url":"https://github.com/myorg/repo.git"}]"#,
            )
            .create();

        let github = create_github(server.url());
        let result = github.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        assert!(matches!(result[0], Err(MirrorError::Skip(_))));
    }

    #[test]
    fn test_get_mirror_repos_parse_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"description":"not valid yaml {{{","url":"https://api.github.com/myorg/repo","ssh_url":"git@github.com:myorg/repo.git","clone_url":"https://github.com/myorg/repo.git"}]"#,
            )
            .create();

        let github = create_github(server.url());
        let result = github.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        assert!(matches!(result[0], Err(MirrorError::Description(_, _))));
    }

    #[test]
    fn test_get_mirror_repos_unauthorized() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(401)
            .with_header("content-type", "application/json")
            .create();

        let github = create_github(server.url());
        let result = github.get_mirror_repos();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unautorized"));
    }

    #[test]
    fn test_get_mirror_repos_invalid_status() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(500)
            .with_header("content-type", "application/json")
            .create();

        let github = create_github(server.url());
        let result = github.get_mirror_repos();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid status"));
    }

    #[test]
    fn test_get_mirror_repos_http_destination() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git","url":"https://api.github.com/myorg/repo","ssh_url":"git@github.com:myorg/repo.git","clone_url":"https://github.com/myorg/repo.git"}]"#,
            )
            .create();

        let github = GitHub {
            url: server.url(),
            org: "myorg".to_string(),
            use_http: true,
            private_token: None,
            useragent: "test".to_string(),
        };
        let result = github.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        let repo = result[0].as_ref().unwrap();
        assert_eq!(repo.destination, "https://github.com/myorg/repo.git");
    }

    #[test]
    fn test_get_mirror_repos_empty_description() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/orgs/myorg/repos")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"description":null,"url":"https://api.github.com/myorg/repo","ssh_url":"git@github.com:myorg/repo.git","clone_url":"https://github.com/myorg/repo.git"}]"#,
            )
            .create();

        let github = create_github(server.url());
        let result = github.get_mirror_repos().unwrap();
        // No description parses as Skip since origin field is missing
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
    }
}
