# Astral Key — multi-stage Rust build
# syntax: docker/dockerfile:1
FROM docker.io/rust:slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo fetch && \
    rm -rf src
COPY . .
RUN cargo build --release

FROM docker.io/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/astral-key /usr/local/bin/astral-key
EXPOSE 8080
USER 1000:1000
CMD ["/usr/local/bin/astral-key"]
