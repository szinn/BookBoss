FROM rust:1@sha256:51c04d7a2b38418ba23ecbfb373c40d3bd493dec1ddfae00ab5669527320195e AS chef

ARG TARGETPLATFORM
ARG TARGETARCH
ARG TARGETOS

RUN apt-get update && apt-get install -y --no-install-recommends musl-tools pkg-config && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked
RUN rustup target add x86_64-unknown-linux-musl

# Install protobuf-compiler
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/DioxusLabs/dioxus/refs/heads/main/.github/install.sh | bash

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder-web
COPY --from=planner /app/recipe.json recipe.json

# Build deps layer (cached)
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY . .

# Build actual binary
RUN /usr/local/cargo/bin/dx bundle --web --package bookboss --release
RUN ls -laR target/dx/bookboss/release/web

FROM chef AS builder-server
COPY --from=planner /app/recipe.json recipe.json

# Build deps layer (cached)
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY . .

# Build actual binary
RUN /usr/local/cargo/bin/dx bundle --server --package bookboss --release --target x86_64-unknown-linux-musl
RUN ls -laR target/dx/bookboss/release/web

# Sanity check: should say "not a dynamic executable"
RUN ldd target/dx/bookboss/release/web/bookboss || true

FROM ubuntu:latest@sha256:f9d633ff6640178c2d0525017174a688e2c1aef28f0a0130b26bd5554491f0da AS certs
RUN addgroup --gid 1000 bookboss && useradd -g 1000 -M -u 1000 -s /usr/sbin/nologin bookboss
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates
RUN update-ca-certificates

# FROM chef AS runtime
FROM scratch
COPY --from=certs /etc/passwd /etc/passwd
COPY --from=certs /etc/group /etc/group
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder-web /app/target/dx/bookboss/release/web/public /app/public
COPY --from=builder-web /app/target/dx/bookboss/release/web/.manifest.json /app
COPY --from=builder-server /app/target/dx/bookboss/release/web/bookboss /app

LABEL tech.zinn.image.target_platform=$TARGETPLATFORM
LABEL tech.zinn.image.target_architecture=$TARGETARCH
LABEL tech.zinn.image.target_os=$TARGETOS

LABEL org.opencontainers.image.source="https://github.com/szinn/BookBoss"
LABEL org.opencontainers.image.description="Take Control Of Your Digital Library"

WORKDIR /app
VOLUME [ /library ]
USER bookboss
ENTRYPOINT [ "/app/bookboss", "server" ]
