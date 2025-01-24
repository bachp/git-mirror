# Build stage
FROM rust:bookworm AS builder
WORKDIR /usr/src/git-mirror

# Copy only necessary files for build
COPY Cargo.toml Cargo.lock .
COPY src src

# Build application
RUN cargo install --path .

# Runtime stage
FROM debian:bookworm-slim

# Install dependencies and clean up in single RUN
RUN apt-get update && \
    apt-get install -y --no-install-recommends git-core git-lfs && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin
COPY --from=builder /usr/local/cargo/bin/git-mirror .
