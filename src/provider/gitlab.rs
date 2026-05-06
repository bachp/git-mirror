/*
 * Copyright (c) 2017-2018 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

// Used for error and debug logging
use log::{debug, error, trace, warn};

use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::provider::{Desc, Mirror, MirrorError, MirrorResult, Provider};

#[derive(Debug)]
pub struct GitLab {
    pub url: String,
    pub group: String,
    pub use_http: bool,
    pub private_token: Option<String>,
    pub recursive: bool,
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
            let url = format!("{url}?per_page={PER_PAGE}&page={page}");
            trace!("URL: {url}");

            let res = client
                .get(&url)
                .headers(headers.clone())
                .send()
                .map_err(|e| format!("Unable to connect to: {url} ({e})"))?;

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
                        trace!("Next page: {n:?}");
                        true
                    }
                }
            };

            let results_page: Vec<T> = serde_json::from_reader(res)
                .map_err(|e| format!("Unable to parse response as JSON ({e})"))?;

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
            match HeaderValue::from_str(token) {
                Ok(token) => {
                    headers.insert("PRIVATE-TOKEN", token);
                }
                Err(err) => {
                    error!("Unable to parse PRIVATE_TOKEN: {err}");
                }
            }
        } else {
            warn!("PRIVATE_TOKEN not set")
        }

        let groups = if self.recursive {
            self.get_subgroups(&self.group, &client, &headers).or_else(
                |e| -> Result<Vec<String>, String> {
                    warn!("Unable to get subgroups: {e}");
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
            match serde_yaml::from_str::<Desc>(&p.description) {
                Ok(desc) => {
                    if desc.skip {
                        mirrors.push(Err(MirrorError::Skip(p.web_url)));
                        continue;
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
                        refspec: desc.refspec,
                        lfs: desc.lfs,
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

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn create_gitlab(server_url: String, recursive: bool) -> GitLab {
        GitLab {
            url: server_url,
            group: "mygroup".to_string(),
            use_http: false,
            private_token: None,
            recursive,
        }
    }

    #[test]
    fn test_gitlab_get_label() {
        let gitlab = create_gitlab("https://gitlab.example.com".to_string(), false);
        assert_eq!(gitlab.get_label(), "https://gitlab.example.com/mygroup");
    }

    #[test]
    fn test_get_mirror_repos_success() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git","web_url":"https://gitlab.example.com/mygroup/repo","ssh_url_to_repo":"git@gitlab.com:mygroup/repo.git","http_url_to_repo":"https://gitlab.com/mygroup/repo.git"}]"#,
            )
            .create();

        let gitlab = create_gitlab(server.url(), false);
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        let repo = result[0].as_ref().unwrap();
        assert_eq!(repo.origin, "https://github.com/test/repo.git");
        assert_eq!(repo.destination, "git@gitlab.com:mygroup/repo.git");
        assert!(repo.lfs);
    }

    #[test]
    fn test_get_mirror_repos_skip() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git\nskip: true\n","web_url":"https://gitlab.example.com/mygroup/repo","ssh_url_to_repo":"git@gitlab.com:mygroup/repo.git","http_url_to_repo":"https://gitlab.com/mygroup/repo.git"}]"#,
            )
            .create();

        let gitlab = create_gitlab(server.url(), false);
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        assert!(matches!(result[0], Err(MirrorError::Skip(_))));
    }

    #[test]
    fn test_get_mirror_repos_parse_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"not valid yaml {{{","web_url":"https://gitlab.example.com/mygroup/repo","ssh_url_to_repo":"git@gitlab.com:mygroup/repo.git","http_url_to_repo":"https://gitlab.com/mygroup/repo.git"}]"#,
            )
            .create();

        let gitlab = create_gitlab(server.url(), false);
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_err());
        assert!(matches!(result[0], Err(MirrorError::Description(_, _))));
    }

    #[test]
    fn test_get_mirror_repos_unauthorized() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(401)
            .with_header("content-type", "application/json")
            .create();

        let gitlab = create_gitlab(server.url(), false);
        let result = gitlab.get_mirror_repos();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unautorized"));
    }

    #[test]
    fn test_get_mirror_repos_invalid_status() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(500)
            .with_header("content-type", "application/json")
            .create();

        let gitlab = create_gitlab(server.url(), false);
        let result = gitlab.get_mirror_repos();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid status"));
    }

    #[test]
    fn test_get_mirror_repos_pagination() {
        let mut server = Server::new();
        let _m1 = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "2")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo1.git","web_url":"https://gitlab.example.com/mygroup/repo1","ssh_url_to_repo":"git@gitlab.com:mygroup/repo1.git","http_url_to_repo":"https://gitlab.com/mygroup/repo1.git"}]"#,
            )
            .create();
        let _m2 = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo2.git","web_url":"https://gitlab.example.com/mygroup/repo2","ssh_url_to_repo":"git@gitlab.com:mygroup/repo2.git","http_url_to_repo":"https://gitlab.com/mygroup/repo2.git"}]"#,
            )
            .create();

        let gitlab = create_gitlab(server.url(), false);
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_mirror_repos_http_destination() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git","web_url":"https://gitlab.example.com/mygroup/repo","ssh_url_to_repo":"git@gitlab.com:mygroup/repo.git","http_url_to_repo":"https://gitlab.com/mygroup/repo.git"}]"#,
            )
            .create();

        let gitlab = GitLab {
            url: server.url(),
            group: "mygroup".to_string(),
            use_http: true,
            private_token: None,
            recursive: false,
        };
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
        let repo = result[0].as_ref().unwrap();
        assert_eq!(repo.destination, "https://gitlab.com/mygroup/repo.git");
    }

    #[test]
    fn test_get_mirror_repos_recursive() {
        let mut server = Server::new();
        let _m1 = server
            .mock("GET", "/api/v4/groups/mygroup/subgroups?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(r#"[{"id": 123}]"#)
            .create();
        let _m2 = server
            .mock("GET", "/api/v4/groups/123/subgroups?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(r#"[]"#)
            .create();
        let _m3 = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo1.git","web_url":"https://gitlab.example.com/mygroup/repo1","ssh_url_to_repo":"git@gitlab.com:mygroup/repo1.git","http_url_to_repo":"https://gitlab.com/mygroup/repo1.git"}]"#,
            )
            .create();
        let _m4 = server
            .mock("GET", "/api/v4/groups/123/projects?per_page=100&page=1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo2.git","web_url":"https://gitlab.example.com/sub/repo2","ssh_url_to_repo":"git@gitlab.com:sub/repo2.git","http_url_to_repo":"https://gitlab.com/sub/repo2.git"}]"#,
            )
            .create();

        let gitlab = create_gitlab(server.url(), true);
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_mirror_repos_with_private_token() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v4/groups/mygroup/projects?per_page=100&page=1")
            .match_header("PRIVATE-TOKEN", "secret123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("x-next-page", "")
            .with_body(
                r#"[{"description":"origin: https://github.com/test/repo.git","web_url":"https://gitlab.example.com/mygroup/repo","ssh_url_to_repo":"git@gitlab.com:mygroup/repo.git","http_url_to_repo":"https://gitlab.com/mygroup/repo.git"}]"#,
            )
            .create();

        let gitlab = GitLab {
            url: server.url(),
            group: "mygroup".to_string(),
            use_http: false,
            private_token: Some("secret123".to_string()),
            recursive: false,
        };
        let result = gitlab.get_mirror_repos().unwrap();
        assert_eq!(result.len(), 1);
    }
}
