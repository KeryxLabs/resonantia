# Resonantia Gateway

Standalone hosted gateway for Resonantia, built on top of the existing `resonantia-core` STTP runtime.

## What It Does

This first scaffold provides:

- `GET /health`
- `GET /api/v1/nodes`
- `POST /api/v1/store`
- `GET /api/v1/graph`
- Compatibility aliases for `/api/nodes`, `/nodes`, `/api/store`, `/store`, `/api/graph`, `/graph`
- Tenant-isolated storage roots under `gateway-data/tenants/<tenant-id>`
- CORS configured for `https://app.resonantia.me` by default

## BYO API Contract

For third-party or self-hosted gateway implementations (including auth-off mode),
see [BYO_GATEWAY_SPEC.md](BYO_GATEWAY_SPEC.md).

## Tenant + Auth Model

Gateway now supports two auth modes:

- `off` (default): header-based tenant selection
- `clerk`: tenant derived from verified Clerk JWT claims

When `RESONANTIA_GATEWAY_AUTH_MODE=off`, tenant selection is header-based:

- `x-resonantia-tenant`
- `x-tenant-id`
- `x-tenant`

If no tenant header is sent, the gateway falls back to `public`.

When `RESONANTIA_GATEWAY_AUTH_MODE=clerk`:

- `Authorization: Bearer <jwt>` is required
- Token is verified against Clerk JWKS
- Tenant resolves from the configured claim (default: `org_id`), then falls back to `sub`

## Environment Variables

- `RESONANTIA_GATEWAY_BIND`
  - default: `0.0.0.0:8090`
- `RESONANTIA_GATEWAY_DATA_DIR`
  - default: `./gateway-data`
- `RESONANTIA_GATEWAY_DEFAULT_TENANT`
  - default: `public`
- `RESONANTIA_GATEWAY_ALLOWED_ORIGINS`
  - default: `https://app.resonantia.me`
  - comma-separated list
- `RESONANTIA_GATEWAY_AUTH_MODE`
  - default: `off`
  - values: `off`, `clerk`
- `RESONANTIA_GATEWAY_CLERK_ISSUER`
  - required when auth mode is `clerk`
  - example: `https://clerk.your-domain.com`
- `RESONANTIA_GATEWAY_CLERK_JWKS_URL`
  - optional
  - default: `<issuer>/.well-known/jwks.json`
- `RESONANTIA_GATEWAY_CLERK_AUDIENCE`
  - optional audience validation
- `RESONANTIA_GATEWAY_CLERK_TENANT_CLAIM`
  - optional
  - default: `org_id`
- `RESONANTIA_GATEWAY_CLERK_JWKS_CACHE_SECONDS`
  - optional
  - default: `300`
- `RESONANTIA_GATEWAY_ALLOW_TENANT_HEADER_FALLBACK`
  - optional
  - default: `false`
  - allows header tenant fallback in clerk mode if token does not carry tenant claim

## Run

From the project root:

```bash
npm run gateway:dev
```

Or directly:

```bash
cargo run --manifest-path gateway/Cargo.toml
```

## Near-Term Next Steps

1. Add explicit allowlist validation for tenant claim format and issuer/audience presets per environment.
2. Add `POST /api/v1/sync` once the hosted sync contract is finalized.
3. Add rate limiting, audit logs, and request IDs.
4. Add managed SurrealDB configuration instead of local per-tenant data dirs.
5. Add admin endpoints for tenant provisioning and usage introspection.
