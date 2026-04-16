#!/bin/sh
# Generate runtime config.js at container startup so one image can run in many environments.
set -eu

js_escape() {
  # Escape characters that can break a JS single-quoted string literal.
  printf '%s' "$1" | sed "s/'/\\\\'/g"
}

BUILD_DIR="${RESONANTIA_BUILD_DIR:-/app/build}"
GATEWAY_URL="$(js_escape "${VITE_GATEWAY_BASE_URL:-}")"
CLERK_KEY="$(js_escape "${VITE_CLERK_PUBLISHABLE_KEY:-}")"
CLERK_TEMPLATE="$(js_escape "${VITE_CLERK_GATEWAY_TOKEN_TEMPLATE:-}")"

write_runtime_config() {
  config_path="$1"
  config_dir="$(dirname "$config_path")"
  mkdir -p "$config_dir"

  cat > "$config_path" <<EOF
// Generated at container startup from environment variables.
window.__resonantia__ = {
  gatewayBaseUrl: '$GATEWAY_URL',
  clerkPublishableKey: '$CLERK_KEY',
  clerkGatewayTokenTemplate: '$CLERK_TEMPLATE',
};
EOF
}

# Always write primary runtime target.
write_runtime_config "$BUILD_DIR/config.js"

# Also update other known served paths when they exist.
if [ -d "/app/.svelte-kit/output/client" ]; then
  write_runtime_config "/app/.svelte-kit/output/client/config.js"
fi

if [ -d "/app/static" ]; then
  write_runtime_config "/app/static/config.js"
fi

exec "$@"
