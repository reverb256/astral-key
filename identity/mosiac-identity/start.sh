#!/bin/sh
# Mosiac Identity — universal start script
# Works on: Linux, macOS, Termux, Windows (Git Bash/WSL), any POSIX env

set -e

echo "╔══════════════════════════════════════════╗"
echo "║        Mosiac Identity Service          ║"
echo "║  Self-sovereign identity for everyone   ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# Check Node.js
if ! command -v node >/dev/null 2>&1; then
  echo "!! Node.js is required. Install it:"
  echo "   Linux:  apt install nodejs npm  (or your package manager)"
  echo "   macOS:  brew install node"
  echo "   Termux: pkg install nodejs"
  echo "   Windows: https://nodejs.org"
  exit 1
fi

echo "  Node.js:  $(node --version)"
echo "  Platform: $(uname -sm 2>/dev/null || echo unknown)"
echo ""

# Install deps if missing
if [ ! -d "node_modules" ]; then
  echo "  Installing dependencies..."
  npm install --omit=dev 2>&1 | tail -3
  echo ""
fi

echo "  Starting server on http://0.0.0.0:${MOSIAC_PORT:-3002}"
echo "  Identity page: http://localhost:${MOSIAC_PORT:-3002}/identity.html"
echo "  API:           http://localhost:${MOSIAC_PORT:-3002}/mosiac/"
echo ""

exec node server.js
