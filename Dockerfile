# Multi-stage build: MIS Rust binary → distroless container
# Build on nexus (zephyr cannot build Rust — OOM risk)

# Stage 1: Build the binary
FROM rust:1.75-slim-bookworm AS builder
WORKDIR /src

# Install build deps
RUN apt-get update && apt-get install -y pkg-config libssl-dev cmake clang && rm -rf /var/lib/apt/lists/*

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/mosaic-identity/Cargo.toml crates/mosaic-identity/

# Build dependencies only (caches layer)
RUN mkdir -p crates/mosaic-identity/src && echo "fn main() {}" > crates/mosaic-identity/src/main.rs && \
    echo "pub mod api; pub mod config; pub mod crypto; pub mod error; pub mod nostr; pub mod storage; pub mod bindings;" > crates/mosaic-identity/src/lib.rs && \
    cargo build -p mosaic-identity --release 2>/dev/null || true

# Copy source and build for real
COPY crates/mosaic-identity/src/ crates/mosaic-identity/src/
COPY crates/mosaic-identity/migrations/ crates/mosaic-identity/migrations/
RUN cargo build -p mosaic-identity --release

# Stage 2: Minimal runtime
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /src/target/release/mosaic-identity /usr/local/bin/mosaic-identity
EXPOSE 8081
VOLUME ["/data"]
ENV MIS_DATABASE_URL=sqlite:///data/mosaic-identity.db?mode=rwc
ENTRYPOINT ["mosaic-identity"]
