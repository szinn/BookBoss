FROM rust:1@sha256:51c04d7a2b38418ba23ecbfb373c40d3bd493dec1ddfae00ab5669527320195e AS chef

ARG TARGETPLATFORM
ARG TARGETARCH
ARG TARGETOS

LABEL tech.zinn.image.target_platform=$TARGETPLATFORM
LABEL tech.zinn.image.target_architecture=$TARGETARCH
LABEL tech.zinn.image.target_os=$TARGETOS

LABEL org.opencontainers.image.source="https://github.com/szinn/BookBoss"
LABEL org.opencontainers.image.description="Take Control Of Your Digital Library"

RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .

# Install protobuf-compiler
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/DioxusLabs/dioxus/refs/heads/main/.github/install.sh | bash
RUN /usr/local/cargo/bin/dx bundle --platform web --package bookboss --release

FROM ubuntu:latest@sha256:f9d633ff6640178c2d0525017174a688e2c1aef28f0a0130b26bd5554491f0da AS ubuntu
RUN addgroup --gid 1000 bookboss && useradd -g 1000 -M -u 1000 -s /usr/sbin/nologin bookboss
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates
RUN update-ca-certificates

FROM chef AS runtime
# FROM scratch
COPY --from=ubuntu /etc/passwd /etc/passwd
COPY --from=ubuntu /etc/group /etc/group
COPY --from=ubuntu /etc/ssl/ /etc/ssl/
COPY --from=builder /app/target/dx/bookboss/release/web/ /app

ENV PORT=8080
ENV IP=0.0.0.0
EXPOSE 8080

WORKDIR /app
VOLUME [ /library ]
USER bookboss
ENTRYPOINT [ "/app/bookboss", "server" ]
