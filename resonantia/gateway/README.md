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

## Tenant Model

For now, tenant selection is header-based:

- `x-resonantia-tenant`
- `x-tenant-id`
- `x-tenant`

If no tenant header is sent, the gateway falls back to `public`.

This is intentional scaffolding, not final auth. The next step is to resolve tenant identity from a signed bearer token instead of trusting raw headers.

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

1. Replace header-based tenant selection with bearer-token auth.
2. Add `POST /api/v1/sync` once the hosted sync contract is finalized.
3. Add rate limiting, audit logs, and request IDs.
4. Add managed SurrealDB configuration instead of local per-tenant data dirs.
5. Add admin endpoints for tenant provisioning and usage introspection.
