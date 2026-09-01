# Stage 1: Build with the official Rust image (ships a C toolchain for any
# crates with build scripts). Nothing else needs installing: the workspace
# pins the toolchain via rust-toolchain.toml and reqwest uses rustls, so no
# OpenSSL headers are required.
FROM rust:1-bookworm AS builder

WORKDIR /app

# Build context is the repo root (see root .dockerignore), so the whole
# workspace — including the path-dependency crates under packages/ — is here.
COPY . .

# --locked: fail instead of silently resolving new versions if Cargo.lock is
# out of date. Only the `api` package (and its dependency graph) is built.
RUN cargo build --release --package api --locked

# Stage 2: Slim Debian runtime — same glibc family as the builder so the
# dynamically linked binary runs unchanged. ca-certificates is needed for any
# outbound HTTPS calls (rustls reads the system root store).
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Only the release binary is copied: no source, no target/, no toolchain.
COPY --from=builder /app/target/release/api /usr/local/bin/api

# Listen on 80 inside the container; Render terminates TLS in front of it.
ENV PORT=80

EXPOSE 80

CMD ["api"]
