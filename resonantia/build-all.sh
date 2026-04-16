#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# build-all.sh — build and optionally push all Resonantia Docker images.
#
# Usage:
#   ./build-all.sh [options]
#
# Options:
#   --tag      <tag>      Image tag (default: from IMAGE_TAG env or "latest")
#   --owner    <owner>    GHCR owner  (default: from GHCR_OWNER env)
#   --push                Push images to GHCR after building
#   --gateway-only        Build only the gateway image
#   --web-only            Build only the web image
#   --help                Show this message
#
# Vite build vars are read from your .env file if present, and can be
# overridden with env vars before running the script:
#   VITE_CLERK_PUBLISHABLE_KEY=pk_live_... ./build-all.sh --tag 1.0.0 --push
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Defaults ──────────────────────────────────────────────────────────────────
TAG="${IMAGE_TAG:-latest}"
OWNER="${GHCR_OWNER:-}"
PUSH=false
BUILD_GATEWAY=true
BUILD_WEB=true

# ── Load .env from repo root if it exists ─────────────────────────────────────
ENV_FILE="$REPO_ROOT/.env"
if [[ -f "$ENV_FILE" ]]; then
  echo "▶ loading $ENV_FILE"
  # Export only lines that look like KEY=VALUE (skip comments and blanks)
  set -o allexport
  # shellcheck disable=SC1090
  source <(grep -E '^[A-Z_]+=.' "$ENV_FILE" | grep -v '^#')
  set +o allexport
fi

# Re-read exported vars after sourcing .env so our defaults can use them
TAG="${IMAGE_TAG:-$TAG}"
OWNER="${GHCR_OWNER:-$OWNER}"

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)        TAG="$2";    shift 2 ;;
    --owner)      OWNER="$2";  shift 2 ;;
    --push)       PUSH=true;   shift   ;;
    --gateway-only) BUILD_WEB=false;     shift ;;
    --web-only)     BUILD_GATEWAY=false; shift ;;
    --help)
      sed -n '/^# ─/,/^# ─/p' "$0"
      exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 1 ;;
  esac
done

# ── Validate ──────────────────────────────────────────────────────────────────
if [[ -z "$OWNER" ]]; then
  echo "error: GHCR_OWNER is not set. Pass --owner <owner> or set GHCR_OWNER in .env" >&2
  exit 1
fi

GATEWAY_IMAGE="ghcr.io/$OWNER/resonantia-gateway:$TAG"
WEB_IMAGE="ghcr.io/$OWNER/resonantia-web:$TAG"

echo ""
echo "  tag    : $TAG"
echo "  owner  : $OWNER"
echo "  push   : $PUSH"
echo "  gateway: $BUILD_GATEWAY  →  $GATEWAY_IMAGE"
echo "  web    : $BUILD_WEB      →  $WEB_IMAGE"
echo ""

# ── Gateway ───────────────────────────────────────────────────────────────────
if [[ "$BUILD_GATEWAY" == true ]]; then
  echo "════════════════════════════════════════"
  echo " building gateway"
  echo "════════════════════════════════════════"
  "$SCRIPT_DIR/gateway/build-image.sh" "$GATEWAY_IMAGE"

  if [[ "$PUSH" == true ]]; then
    echo "▶ pushing $GATEWAY_IMAGE"
    docker push "$GATEWAY_IMAGE"
    echo "✓ gateway pushed"
  fi
fi

# ── Web ───────────────────────────────────────────────────────────────────────
if [[ "$BUILD_WEB" == true ]]; then
  echo "════════════════════════════════════════"
  echo " building web"
  echo "════════════════════════════════════════"

  docker build \
    --tag "$WEB_IMAGE" \
    "$SCRIPT_DIR"

  echo "✓ web image built: $WEB_IMAGE"

  if [[ "$PUSH" == true ]]; then
    echo "▶ pushing $WEB_IMAGE"
    docker push "$WEB_IMAGE"
    echo "✓ web pushed"
  fi
fi

echo ""
echo "✓ all done"
