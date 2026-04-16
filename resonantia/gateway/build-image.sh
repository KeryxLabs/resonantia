#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-ghcr.io/keryxlabs/resonantia-gateway:latest}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "▶ building resonantia-gateway (release)..."
(cd "$SCRIPT_DIR" && cargo build --release)

echo "▶ staging binary..."
mkdir -p "$SCRIPT_DIR/publish"
cp "$SCRIPT_DIR/target/release/resonantia-gateway" "$SCRIPT_DIR/publish/"

echo "▶ building docker image: $TAG"
docker build -t "$TAG" "$SCRIPT_DIR"

echo "✓ done — $TAG"
