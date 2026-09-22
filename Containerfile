# Astral Key — multi-stage Rust build
FROM docker.io/rust:slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config libssl-dev cmake clang && \
    rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release -p astral-key

FROM docker.io/debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/astral-key /usr/local/bin/astral-key
EXPOSE 8080
USER 1000:1000
CMD ["/usr/local/bin/astral-key"]
