# ── Mosiac Identity — multi-arch OCI image ──────────────
# Supports: linux/amd64, linux/arm64, linux/arm/v7
#
# Build for your platform:
#   docker build -f oci/Dockerfile -t mosiac-identity .
#
# Build for all platforms (requires buildx):
#   docker buildx build -f oci/Dockerfile \
#     --platform linux/amd64,linux/arm64,linux/arm/v7 \
#     -t ghcr.io/reverb256/mosiac-identity:latest --push .
#
# Quick run:
#   docker run -d -p 3002:3002 -v mosiac-data:/data mosiac-identity

FROM node:22-alpine
RUN apk add --no-cache tini
WORKDIR /app

# Copy manifest first (layer caching)
COPY package*.json ./
RUN npm ci --omit=dev
# ^ --ignore-scripts skips better-sqlite3 build if it fails

# Copy source
COPY server.js start.sh ./
COPY src/ src/
COPY i18n/ i18n/
COPY public/ public/

ENV MOSIAC_PORT=3002
EXPOSE 3002

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD wget -qO- http://127.0.0.1:${MOSIAC_PORT:-3002}/health || exit 1

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["node", "server.js"]
