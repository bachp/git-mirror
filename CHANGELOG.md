# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Don't report skipped tasks as YAML parser error.

## [0.14.2] - 2020-09-23

### Fixed
- --version now prints the correct name
- Properly handle `no_proxy` (reqwest 0.10.8)

## [0.14.1] - 2020-04-19

### Fixed
- Fix generation of release binaries for linux, macos and windows.

### Changed
- Native TLS support now enabled for reqwest
- The linux binaries now use a vendored static versions of openssl.

### Removed
- Builds for other systems than 64-bit Linux, Windows and MacOS removed.

## [0.14.0] - 2020-04-14

### Removed
- The deprecated `GITLAB_PRIVATE_TOKEN` is not longer recognized. Use `PRIVATE_TOKEN` instead.

### Changed
- Commandline parsing changed from clap to structopt.
- Updated dependencies

## [0.13.1] - 2019-10-19

### Changed
- Update dependencies

## [0.13.0] - 2019-08-29

### Added
- Better progress reporting by adding index and total in START and END messages

## [0.12.0] - 2019-04-24

### Added
- Allow logging to be configured via the `RUST_LOG` variable of  [`env_logger`](https://crates.io/crates/env_logger)
- Add posibility to set the global default `refspec` via command line argument `--refspec`. This will be used if no repo specific
  refspec is given.

### Changed
- Change job end marker from `OK` -> `END(OK)` and `FAIL` -> `END(FAIL)`. This allows easier parsing.

## [0.11.0] - 2019-03-20
### Added
- Allow specifying a `refspec` for things to push to the destination path.

### Changed
- Use Rust 2018 edition
- Update dependencies

## [0.10.0] - 2018-11-22
### Added
- Add parameter to sumarize the results in a JUnit XML compatible format. This format
can be parsed by many reporting tools, including Gitlab CI and Jenkins.

### Changed
- Update to reqwest 0.9. This makes git-mirror compatible with OpenSSL 1.1.1.
- Update dependencies

## [0.9.1] - 2018-08-24
### Changed
- Update dependencies

### Fixed
- Automatically find OpenSSL certificates by searching in different known paths.
  This can be overriden manually by setting the `SSL_CERT_DIR` and `SSL_CERT_FILE`
  enivronment variables.

## [0.9.0] - 2018-08-20
### Fixed
- Provider selection now working correctly

### Changed
- Binary size reduced by using LTO
- Hyper replaced by reqwest

### Removed
- RusTLS is no longer supported as a TLS provider

### Deprecated
- Environment variable `GITLAB_PRIVATE_TOKEN`, replaced by `PRIVATE_TOKEN`

## [0.8.0] - 2018-03-26
### Changed
- Improved error logging for git commands
- Updated dependencies
- Docker image now uses native-tls instead of RusTLS

## [0.7.1] - 2018-03-04
### Fixed
- Fix compatibility with Gitlabl < 10.3

## [0.7.0] - 2018-01-27
### Added
- GitLab subgroups support
- Docker container

### Changed
- Updated dependencies

## [0.6.0] - 2017-07-23
### Changed
- Prometheus metrics contain mirror label to support multiple git-mirror jobs per machine

## [0.5.0] - 2017-07-19
### Added
- Support exporting metrics via Prometheus via textfile collector

## [0.4.0] - 2017-07-12
### Changed
- Allow only one instance per mirror directory

## [0.3.0] - 2017-07-11
### Added
- Fetch all projects from gitlab using pagination
- Add timestamp for logs on stderr

### Changed
- Improve output to stdout for parallel output

## [0.2.3] - 2017-07-10
### Changed
- Don't hardcode path to git binary

## [0.2.2] - 2017-07-10
### Fixed
- Fix issue with non existing working directory

## [0.2.1] - 2017-07-08
### Added
- Add Travis CI support
- Update dependencies

## [0.2.0] - 2017-07-03
### Added
- Add support for Github

### Changed
- Use RusTLS by default

## [0.1.0] - 2017-06-17
### Added
- Inital Releas
- Support GitLab
