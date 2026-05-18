# syntax=docker/dockerfile:1.7
#
# Multi-stage build for `diaspor-api-server`.
#
# Stage 1 (`chef`/`planner`/`builder`) — uses the cargo-chef pattern so the
# dependency graph compiles once and is cached as its own layer. Source
# code changes invalidate only the final `cargo build` step, not the
# 15-minute deps build.
#
# Stage 2 (`runtime`) — distroless `cc` image (just glibc + ca-certs), no
# shell, runs as the non-root `nonroot` user (UID/GID 65532). The
# distroless image has no curl/wget so a Dockerfile `HEALTHCHECK` is
# intentionally omitted; Kubernetes runs liveness / readiness probes
# against `/v1/health` instead — see
# `deploy/helm/diaspor-api-server/values.yaml`.
#
# Build context MUST be the repo root (the workspace `Cargo.toml` lives
# there), e.g.:
#
#     docker build \
#         -f deploy/docker/diaspor-api-server/Dockerfile \
#         -t diaspor-api-server:dev .
#
# Run locally:
#
#     docker run --rm -p 7733:7733 \
#         -e DIASPOR_JWT_SECRET=devsecret \
#         diaspor-api-server:dev

# ---------------------------------------------------------------------------
# Stage 1a: chef base — pulls cargo-chef once, reused by planner + builder.
# ---------------------------------------------------------------------------
FROM rust:1.85-bookworm AS chef
WORKDIR /build
RUN cargo install cargo-chef --locked --version ^0.1

# ---------------------------------------------------------------------------
# Stage 1b: planner — generate the dependency recipe.
# ---------------------------------------------------------------------------
FROM chef AS planner
# Copy the whole workspace so `cargo chef prepare` sees every member's
# Cargo.toml (the workspace has 15 crates — see crates/ for the list).
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# ---------------------------------------------------------------------------
# Stage 1c: builder — cook deps (cached layer), then build the binary.
# ---------------------------------------------------------------------------
FROM chef AS builder

# Cook only the dependency graph. This layer is the cached one; it only
# rebuilds when recipe.json (= the union of every Cargo.toml in the
# workspace) changes.
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p diaspor-api-server

# Now copy the actual source and build the binary. Only this layer (and
# below) rebuilds on code changes.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p diaspor-api-server --bin diaspor-api-server \
    && strip target/release/diaspor-api-server || true

# ---------------------------------------------------------------------------
# Stage 2: runtime — distroless, non-root.
# ---------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

# OCI image metadata. Tools like `docker inspect`, GitHub Container
# Registry, and Kubernetes admission controllers read these labels.
# Build-time override: pass --build-arg IMAGE_VERSION=v0.x.y so the image
# label matches the release tag. Workflow `release-heavy.yml` passes the
# release tag here.
ARG IMAGE_VERSION=0.1.0-alpha.1

LABEL org.opencontainers.image.title="Diaspor API Server" \
      org.opencontainers.image.description="Hosted HTTP + WebSocket API server for Diaspor non-verbal video AI (api.diaspor.io)." \
      org.opencontainers.image.source="https://github.com/stonyp90/diaspor" \
      org.opencontainers.image.url="https://github.com/stonyp90/diaspor" \
      org.opencontainers.image.documentation="https://github.com/stonyp90/diaspor/blob/main/deploy/README.md" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.version="${IMAGE_VERSION}" \
      org.opencontainers.image.vendor="Anthony Paquet" \
      org.opencontainers.image.authors="anthonypaquet1508@gmail.com"

# The container binds 0.0.0.0:7733 so it's reachable from outside the
# pod / container network. Local-dev (no env var set) keeps loopback-only.
ENV DIASPOR_BIND_ADDR=0.0.0.0:7733 \
    RUST_LOG=diaspor_api_server=info,axum=info,tower=info

COPY --from=builder /build/target/release/diaspor-api-server /usr/local/bin/diaspor-api-server

EXPOSE 7733

USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/diaspor-api-server"]
