# Git Mirror

Git Mirror will watch a GitLab or GitHub groups and keep it in sync with external git repositories.

## Usage

1. Create group on your gitlab instance or gitlab.com. e.g. `mirror-test`
2. Add a repository you like to sync to. e.g `my-project`
3. Add a description to the project in YAML format containing an `origin` field. e.g. `origin: https://git.example.org/my-project.git`
4. Execute  set the `PRIVATE_TOKEN` environment variable a personal access token or your private token and execute `git-mirror`

``` sh
export PRIVATE_TOKEN="<personal-access-token>"
git-mirror -g mirror-test
```

This will sync the group `mirror-test` on gitlab.com. If you want to sync a group on a different GitLab instance, use the `-u` flag.

``` sh
git-mirror -g mirror-test -u http://gitlab.example.org
```

### Multiple concurrent jobs

`git-mirror` allows to execute multiple mirror jobs in parallel using the `-c <n>` flag.

``` sh
git-mirror -g mirror-test -c 8
```

This will execute at most 8 sync jobs in parallel

### Description format

For `git-mirror` to mirror a repository it needs to know where to sync from.
In order to achive this `git-mirror` expects the description field of a mirrored project to
be valid [YAML](http://yaml.org/) with at least an `origin` field.

``` yaml
origin: https://git.example.org/my-project.git
```

A list of currently supported fields

- `origin` Source repository to mirror from
- `skip`   Temporarily exclude a project from syncing by adding `skip: true`
- `destination` Reserved for future use

Any other fields are ignored

### Mirror to GitHub

`git-mirror` also supports mirroring to GitHub.

This can be done by specifying GitHub as provider:

``` sh
export PRIVATE_TOKEN="<personal-access-token>"
git-mirror -g mirror-test -p GitHub
```

This has been tested against github.com but it might also work with on premise installations of GitHub.

## Docker

There is also a docker image available. It can be used as follows:

```
docker run -e PRIVATE_TOKEN="x" bachp/git-mirror git-mirror -g mirror -u http://gitlab.example.com
```

## Building & Installing

In order to build this project you need a least rust v1.18.0. The easiest way to get rust is via: [rustup.rs](http://rustup.rs/)

The project can be built using cargo

```
cargo build
```

## They're using Git Mirror

* [gitlab-mirror-orchestrator](https://gitlab.ow2.org/ow2/gitlab-mirror-orchestrator) tool at OW2 Consortium.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details
