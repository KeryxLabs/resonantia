# Resonantia BYO Gateway API Spec

This document defines the HTTP contract expected by Resonantia web/desktop clients
for Bring Your Own (BYO) gateway usage.

The goal is compatibility for custom gateways and OSS alternatives (for example
`sttp-gateway`) while preserving support for legacy route aliases.

## Compatibility Levels

- Minimum compatible (sync only):
  - `GET /api/v1/nodes`
  - `POST /api/v1/store`
  - Optional aliases for older clients:
    - `GET /api/nodes`, `GET /nodes`
    - `POST /api/store`, `POST /store`

- Full hosted-compatible:
  - Sync routes above, plus:
  - `GET /health`
  - `GET /api/v1/graph` (and aliases)
  - `GET /api/v1/account`
  - `POST /api/v1/checkout`
  - `POST /api/v1/customer-portal`
  - `POST /stripe/webhook`

## Auth Modes

Two auth styles are supported by official gateway behavior:

- Auth off (BYO/no-auth):
  - No `Authorization` header required.
  - Tenant can be selected by headers (`x-resonantia-tenant`, `x-tenant-id`, `x-tenant`),
    otherwise default tenant is used.

- Clerk auth:
  - `Authorization: Bearer <jwt>` required.
  - JWT validated against configured issuer/JWKS.

Important: BYO clients may send no auth token when none is configured. Gateway must accept
that in auth-off mode.

## CORS Requirements (Browser BYO)

For browser-based clients, BYO gateway must allow the app origin.

Required behavior:

- `Access-Control-Allow-Origin: <app-origin>` (or `*` for temporary testing)
- `Access-Control-Allow-Methods` includes `GET, POST, PATCH, OPTIONS`
- `Access-Control-Allow-Headers` includes at least `Content-Type, Authorization`
- Respond to `OPTIONS` preflight

If CORS is missing, browser may show status 200 in network but still block response.

## Endpoint Contract

## `GET /health`

Purpose: transport/health probe.

Response 200 JSON:

```json
{
  "status": "ok",
  "transport": "sttp-core-rs (...)"
}
```

## `GET /api/v1/nodes`

Legacy aliases:

- `GET /api/nodes`
- `GET /nodes`

Query params:

- `limit` (optional integer)
  - If omitted, server default applies.
- `sessionId` (optional string)

Note: `limit` is optional. Clients may call without params.

Response 200 JSON:

```json
{
  "nodes": [
    {
      "raw": "...sttp node text...",
      "sessionId": "session-name",
      "tier": "raw",
      "timestamp": "2026-04-16T00:00:00Z",
      "compressionDepth": 2,
      "parentNodeId": null,
      "userAvec": {
        "stability": 0.7,
        "friction": 0.2,
        "logic": 0.9,
        "autonomy": 0.8,
        "psi": 2.6
      },
      "modelAvec": {
        "stability": 0.8,
        "friction": 0.2,
        "logic": 0.9,
        "autonomy": 0.9,
        "psi": 2.8
      },
      "compressionAvec": null,
      "rho": 0.9,
      "kappa": 0.9,
      "psi": 2.7,
      "syncKey": "...",
      "syntheticId": "..."
    }
  ],
  "retrieved": 1
}
```

## `POST /api/v1/store`

Legacy aliases:

- `POST /api/store`
- `POST /store`

Request JSON (camelCase):

```json
{
  "node": "<sttp node text>",
  "sessionId": "session-name"
}
```

Response 200 JSON:

```json
{
  "nodeId": "...",
  "psi": 2.7,
  "valid": true,
  "validationError": null,
  "duplicateSkipped": false,
  "upsertStatus": "created"
}
```

`upsertStatus` should be one of:

- `created`
- `updated`
- `duplicate`
- `skipped`

## `GET /api/v1/graph`

Legacy aliases:

- `GET /api/graph`
- `GET /graph`

Query params:

- `limit` (optional)
- `sessionId` (optional)

Response 200 JSON:

```json
{
  "sessions": [],
  "nodes": [],
  "edges": [],
  "retrieved": 0
}
```

## Billing/Account Endpoints (Hosted-compatible)

## `GET /api/v1/account`

Response 200 JSON:

```json
{
  "userId": "user_...",
  "tier": "free",
  "memberSince": "2026-04-16T00:00:00Z"
}
```

## `POST /api/v1/checkout`

Request JSON:

```json
{
  "tier": "resonant"
}
```

`tier` must be `resonant` or `soulful`.

Response 200 JSON:

```json
{
  "url": "https://checkout.stripe.com/..."
}
```

## `POST /api/v1/customer-portal`

Response 200 JSON:

```json
{
  "url": "https://billing.stripe.com/..."
}
```

## Error Format

Use structured JSON errors with meaningful status codes.

Preferred shape:

```json
{
  "error": "human-readable message"
}
```

Typical statuses:

- `400` bad request (invalid input)
- `401` unauthorized
- `403` forbidden
- `404` not found
- `500` internal error

## Gateway URL Semantics

BYO URL should represent gateway base, not a specific endpoint.

Examples:

- Good: `http://10.12.0.11:8090`
- Good: `http://10.12.0.11:8090/api/v1` (clients normalize this)
- Avoid storing endpoint-specific URLs like `/api/v1/nodes` as base.

## Legacy Compatibility Notes

Clients normalize and attempt legacy aliases for store/nodes/graph paths.
Implementing aliases is recommended for broad compatibility.

## Reference Behavior Summary

- `GET /api/v1/nodes` can be called with or without query params.
- No auth header should be required in auth-off BYO mode.
- Browser CORS must be enabled for app origin when using direct cross-origin BYO calls.
