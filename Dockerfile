FROM rust:stretch
WORKDIR /usr/src/git-mirror
COPY . .
RUN cargo install

FROM debian:stretch-backports
RUN apt-get update && apt-get install -t stretch-backports -y git-core && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=0 /usr/local/cargo/bin/git-mirror .
