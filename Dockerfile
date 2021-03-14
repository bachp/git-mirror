FROM rust:buster
WORKDIR /usr/src/git-mirror
COPY . .
RUN cargo install --path .

FROM debian:buster
RUN apt-get update && apt-get install -y git-core && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=0 /usr/local/cargo/bin/git-mirror .
