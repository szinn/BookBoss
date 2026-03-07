FROM rust:1@sha256:51c04d7a2b38418ba23ecbfb373c40d3bd493dec1ddfae00ab5669527320195e AS chef

# ARG TARGETPLATFORM
# ARG TARGETARCH
# ARG TARGETOS

RUN apt-get update && apt-get install -y --no-install-recommends musl-tools pkg-config && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked
RUN rustup target add x86_64-unknown-linux-musl

# Install protobuf-compiler
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/DioxusLabs/dioxus/refs/heads/main/.github/install.sh | bash

RUN curl -fsSL -o /usr/local/bin/tailwindcss \
    https://github.com/tailwindlabs/tailwindcss/releases/download/v4.2.1/tailwindcss-linux-x64 && \
    chmod +x /usr/local/bin/tailwindcss

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder-web
COPY --from=planner /app/recipe.json recipe.json

# Build deps layer (cached)
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY . .

RUN tailwindcss -i ./crates/frontend/assets/input.css -o ./crates/frontend/assets/tailwind.css

# Build actual binary
RUN /usr/local/cargo/bin/dx bundle --web --package bookboss --release

FROM chef AS builder-server
COPY --from=planner /app/recipe.json recipe.json

# Build deps layer (cached)
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

COPY . .

RUN tailwindcss -i ./crates/frontend/assets/input.css -o ./crates/frontend/assets/tailwind.css

# Build actual binary
RUN /usr/local/cargo/bin/dx bundle --server --package bookboss --release --target x86_64-unknown-linux-musl

# Sanity check: should say "not a dynamic executable"
RUN ldd target/dx/bookboss/release/web/bookboss || true

FROM ubuntu:latest@sha256:d1e2e92c075e5ca139d51a140fff46f84315c0fdce203eab2807c7e495eff4f9 AS certs
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

# LABEL tech.zinn.image.target_platform=$TARGETPLATFORM
# LABEL tech.zinn.image.target_architecture=$TARGETARCH
# LABEL tech.zinn.image.target_os=$TARGETOS

LABEL org.opencontainers.image.source="https://github.com/szinn/BookBoss"
LABEL org.opencontainers.image.description="Take Control Of Your Digital Library"

WORKDIR /app
VOLUME [ /library ]
USER bookboss
ENTRYPOINT [ "/app/bookboss", "server" ]
