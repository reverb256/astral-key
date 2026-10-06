# Mosaic Identity Service (MIS) — the PKI sidecar, NOT astral-key.
#
# Use the `Containerfile` for the astral-key service. This Dockerfile builds
# `mosaic-identity`, a different binary listening on 8081. CI used to default to
# this file and go green while producing a mosaic-identity image tagged as
# astral-key; the Containerfile is now pinned explicitly there.
#
# Build on nexus — zephyr has ~12 GB RAM and is cordoned, so a Rust release
# build there OOMs.

# Stage 1: Build the binary
#
# Toolchain: 1.85 is the floor, and it is a transitive-dependency floor, not a
# preference. `Cargo.lock` resolves `clap_lex` 1.1.1, whose manifest requires the
# `edition2024` Cargo feature — stabilized in Cargo 1.85. On 1.75 the build dies
# with:
#
#   error: failed to parse manifest at .../clap_lex-1.1.1/Cargo.toml
#   feature `edition2024` is required
#   ... not stabilized in this version of Cargo (1.75.0)
#
# Our own crates are edition 2021 and unaffected; this is a dependency's floor.
# 1.88 is used rather than the rolling `slim-bookworm` so the builder image is
# reproducible — a floating tag means an unrelated cargo release can break this
# build with no change on our side, which is how the 1.75 pin rotted unnoticed.
FROM docker.io/library/rust:1.88-slim-bookworm AS builder
WORKDIR /src

RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config libssl-dev cmake clang && \
    rm -rf /var/lib/apt/lists/*

# The whole workspace, in one copy.
#
# This list was previously hand-rolled as `COPY Cargo.toml Cargo.lock ./` plus
# only `crates/mosaic-identity/{src,migrations}/`, which produced a broken
# workspace in /src and failed with:
#
#   error: failed to parse manifest at /src/Cargo.toml
#   no targets specified in the manifest
#   either src/lib.rs, src/main.rs, a [lib] section, or [[bin]] section must be present
#
# The cause was not a broken crate — mosaic-identity compiles cleanly on the full
# tree (verified 2026-10-06: "Finished release profile in 1m 49s", warnings only).
# Cargo could not find a target because the root `Cargo.toml` declares a
# `[workspace]` over 11 members, and a partially-copied tree leaves those members
# missing, so the workspace root resolves to nothing. Copying the tree whole is
# what fixes it.
#
# `.dockerignore` keeps this cheap: it strips the multi-GB `target/` and the
# non-build directories, so `COPY . .` sends ~1 MB rather than ~6 GB.
COPY . .

# Build dependency-only artifacts first so the layer caches across source edits.
# `|| true` is deliberate: this step is a cache warmer, and the real build below
# is the one that must succeed.
RUN cargo build -p mosaic-identity --release --package mosaic-identity 2>/dev/null || true

# The authoritative build — no error suppression.
RUN cargo build -p mosaic-identity --release

# Stage 2: Minimal runtime
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /src/target/release/mosaic-identity /usr/local/bin/mosaic-identity
EXPOSE 8081
VOLUME ["/data"]
ENV MIS_DATABASE_URL=sqlite:///data/mosaic-identity.db?mode=rwc
ENTRYPOINT ["mosaic-identity"]
