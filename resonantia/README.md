# Resonantia

A local-first, privacy-respecting app for capturing, calibrating, and rehydrating AI conversation context using the [STTP protocol](https://github.com/keryxlabs/sttp-core-rs). Built with Tauri + SvelteKit + Rust.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Resonantia App                           │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  SvelteKit UI  (Svelte 5, SPA via adapter-static)        │   │
│  │                                                          │   │
│  │  src/lib/Weaver.svelte       — main canvas + state       │   │
│  │  src/lib/cloudAuth.ts        — Clerk JS integration      │   │
│  │  src/lib/resonantiaClient.ts — thin client shim          │   │
│  │  packages/resonantia-core/   — types, client interface   │   │
│  │  packages/resonantia-ui/     — shared Svelte components  │   │
│  └────────────────────────┬─────────────────────────────────┘   │
│                           │ Tauri invoke / fetch                │
│  ┌────────────────────────▼─────────────────────────────────┐   │
│  │  Tauri Rust Core  (src-tauri/)                           │   │
│  │                                                          │   │
│  │  resonantia-core/src/lib.rs  — all Tauri commands        │   │
│  │  Local SurrealDB (surrealkv) — per-device node storage   │   │
│  │  STTP runtime                — calibration, compress     │   │
│  │  Ollama HTTP client          — local AI model calls      │   │
│  └────────────────────────┬─────────────────────────────────┘   │
└───────────────────────────┼─────────────────────────────────────┘
                            │ HTTPS (Bearer JWT)
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│  Resonantia Gateway  (gateway/)                                 │
│                                                                 │
│  Axum REST API                                                  │
│  Clerk JWT verification (RS256, JWKS cached)                    │
│  Per-user tenant storage — gateway-data/tenants/<user_id>/      │
│  Account DB (surrealkv)  — gateway-data/accounts/               │
└─────────────────────────────────────────────────────────────────┘
```

### Data Tiers

| Layer | Storage | Who controls it |
|---|---|---|
| Local device | SurrealDB (surrealkv file, in-app-data) | User |
| Hosted cloud | SurrealDB (surrealkv, per-user tenant dir on gateway) | Resonantia (you) |
| Self-hosted | Same gateway, user's own server | Power user |

The app is fully functional with no network connection. Cloud sync is opt-in via the settings drawer.

---

## Repository Layout

```
resonantia/               ← this repo
├── src/                  ← SvelteKit app source
│   ├── lib/
│   │   ├── Weaver.svelte           main UI + all state
│   │   ├── cloudAuth.ts            Clerk JS + account fetch
│   │   ├── resonantiaClient.ts     Tauri / web client shim
│   │   └── components/
│   │       ├── SettingsDrawer.svelte
│   │       ├── SyncCloudStatus.svelte
│   │       ├── CalibrateDrawer.svelte
│   │       ├── ComposeDrawer.svelte
│   │       ├── TelescopePanel.svelte
│   │       └── ...
│   └── routes/
│       └── +page.svelte            single route → <Weaver />
│
├── packages/
│   ├── resonantia-core/  ← @resonantia/core (TS types + client interface)
│   └── resonantia-ui/    ← @resonantia/ui   (shared Svelte components)
│
├── src-tauri/
│   ├── tauri.conf.json
│   ├── src/lib.rs                  Tauri command bindings
│   └── resonantia-core/
│       └── src/lib.rs              full Rust business logic, SurrealDB, STTP
│
├── gateway/
│   ├── Cargo.toml
│   └── src/main.rs                 Axum REST gateway
│
├── static/               ← swipe.js, sw.js, theme.js, manifest
├── svelte.config.js
├── vite.config.js        ← dev gateway proxy plugin lives here
├── LAN_DEPLOY.md
└── package.json
```

---

## Tech Stack

| Layer | Tech |
|---|---|
| Desktop app shell | Tauri 2 |
| Frontend | SvelteKit 2 (Svelte 5), SPA mode, adapter-static |
| Local database | SurrealDB 3 — surrealkv (Tauri) / IndexedDB/mem (web) |
| STTP runtime | `sttp-core-rs` crate |
| Auth | Clerk (clerk-js on frontend, JWKS verification in gateway) |
| Gateway | Axum 0.8, Tokio |
| Gateway database | SurrealDB 3 — surrealkv |
| AI model | Ollama (local HTTP, configurable endpoint + model) |
| AI summaries | Same Ollama endpoint, preamble-guided |

---

## User Tiers

### Normie path (default)
- App opens, local SurrealDB initialises automatically
- Everything stored on device, zero config required
- Local Ollama runs AI features (to be hooked up)
- Settings → **Resonantia Account** → "sign in to resonantia" → Clerk modal → done
- Token auto-refreshes, sync starts immediately

### Power user path
- Settings → **Advanced settings** → override Ollama server + model
- Override **Gateway URL** to point at self-hosted gateway instead of the hosted service
- Manual auth token field available for debugging

---

## Environment Variables

### Frontend (Vite / Tauri build)

| Variable | Required | Description |
|---|---|---|
| `VITE_CLERK_PUBLISHABLE_KEY` | For cloud sync | Clerk publishable key |
| `VITE_CLERK_GATEWAY_TOKEN_TEMPLATE` | Optional | Clerk session template name for gateway JWTs |
| `VITE_GATEWAY_BASE_URL` | Optional | Default gateway URL baked into web/Tauri builds — normies auto-point here |
| `VITE_ALLOWED_HOSTS` | Optional | Comma-separated extra allowed dev hosts |

For Tauri builds, `VITE_GATEWAY_BASE_URL` is also available as `GATEWAY_BASE_URL` at Rust compile time via `option_env!("GATEWAY_BASE_URL")`.

### Gateway

| Variable | Default | Description |
|---|---|---|
| `RESONANTIA_GATEWAY_BIND` | `0.0.0.0:8090` | Bind address |
| `RESONANTIA_GATEWAY_DATA_DIR` | `./gateway-data` | Root data directory |
| `RESONANTIA_GATEWAY_DEFAULT_TENANT` | `public` | Fallback tenant when auth is off |
| `RESONANTIA_GATEWAY_ALLOWED_ORIGINS` | `https://app.resonantia.me` | CORS origins, comma-separated |
| `RESONANTIA_GATEWAY_AUTH_MODE` | `off` | `off` or `clerk` |
| `RESONANTIA_GATEWAY_ADMIN_SECRET` | — | Secret for `PATCH /api/v1/account/tier` (webhook handler) |
| `RESONANTIA_GATEWAY_CLERK_ISSUER` | — | Required when mode is `clerk` — e.g. `https://clerk.resonantia.me` |
| `RESONANTIA_GATEWAY_CLERK_JWKS_URL` | `<issuer>/.well-known/jwks.json` | Override JWKS endpoint |
| `RESONANTIA_GATEWAY_CLERK_AUDIENCE` | — | Optional JWT audience validation |
| `RESONANTIA_GATEWAY_CLERK_TENANT_CLAIM` | `org_id` | JWT claim used as tenant ID (falls back to `sub`) |
| `RESONANTIA_GATEWAY_CLERK_JWKS_CACHE_SECONDS` | `300` | How long to cache JWKS keys |
| `RESONANTIA_GATEWAY_ALLOW_TENANT_HEADER_FALLBACK` | `false` | Allow header-based tenant fallback in Clerk mode |

---

## Gateway API

All node/sync routes are per-tenant (scoped to the authenticated user's storage).

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/health` | optional | Health check |
| `GET` | `/api/v1/account` | Clerk JWT | Get account info (tier, memberSince) |
| `PATCH` | `/api/v1/account/tier` | `X-Admin-Secret` | Update user tier (`free`\|`subscriber`) |
| `POST` | `/api/v1/store` | Clerk JWT | Store an STTP node |
| `GET` | `/api/v1/nodes` | Clerk JWT | List nodes (`?limit=&session_id=`) |
| `GET` | `/api/v1/graph` | Clerk JWT | Graph view (`?limit=&session_id=`) |

Aliases without `/v1/` exist for all store/nodes/graph routes.

### Account endpoint response

```json
{ "userId": "user_xxx", "tier": "free", "memberSince": "2026-04-15T..." }
```

### Tier update (webhook / admin)

```bash
curl -X PATCH https://gateway.resonantia.me/api/v1/account/tier \
  -H "X-Admin-Secret: $RESONANTIA_GATEWAY_ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{ "user_id": "user_xxx", "tier": "subscriber" }'
```

### Auth model (Clerk mode)

- JWT `sub` claim → **user identity** (account record key)
- JWT `org_id` (or configured `RESONANTIA_GATEWAY_CLERK_TENANT_CLAIM`) → **tenant ID** (storage bucket)
- For solo users without orgs, `sub` is used as both identity and tenant
- Account auto-provisions on first authenticated request (no separate sign-up call needed)

---

## Data Schema

### Account store (`gateway-data/accounts/`)

SurrealDB namespace `resonantia`, database `accounts`.

```
account {
  user_id    string  // Clerk sub — unique index
  created_at string  // ISO 8601
  tier       string  // 'free' | 'subscriber'
}
```

Only three fields. No email, no name, no PII beyond the Clerk user ID.

### Tenant node store (`gateway-data/tenants/<tenant_id>/`)

One surrealkv database per tenant, namespaced by SurrealDB namespace `resonantia` / db `local`. Schema managed entirely by `sttp-core-rs`. Tables: `temporal_node`, `app_config`, `calibration_state`.

---

## Local Development

### Prerequisites

- Rust stable toolchain (`rustup`)
- Node.js 20+
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS
- Ollama (optional, for AI features)

### 1 — Install dependencies

```bash
cd resonantia
npm install
```

### 2 — Environment

Create `.env` in `resonantia/` (gitignored):

```env
VITE_CLERK_PUBLISHABLE_KEY=pk_test_...
VITE_CLERK_GATEWAY_TOKEN_TEMPLATE=resonantia-gateway   # optional
VITE_GATEWAY_BASE_URL=http://localhost:8090             # local gateway
```

### 3 — Run the gateway

```bash
npm run gateway:dev
```

Starts Axum on `http://localhost:8090`. In dev (`RESONANTIA_GATEWAY_AUTH_MODE` unset → `off`), no JWT is required — tenant is set by header or defaults to `public`.

### 4a — Run as Tauri desktop app

```bash
npm run tauri dev
```

Hot-reloads the Svelte frontend. The Tauri Rust core rebuilds on Rust changes.

### 4b — Run as web app (browser)

```bash
npm run dev
```

Opens on `http://localhost:1420`. Uses the `webClient` (IndexedDB / in-memory SurrealDB).

### 4c — LAN testing (mobile / other devices)

```bash
npm run deploy:lan
```

Builds and serves on `0.0.0.0:4173`. See [LAN_DEPLOY.md](LAN_DEPLOY.md) for full guide.

### Type checking

```bash
npm run check
```

---

## Building for Production

### Tauri desktop

```bash
VITE_CLERK_PUBLISHABLE_KEY=pk_live_... \
VITE_GATEWAY_BASE_URL=https://gateway.resonantia.me \
GATEWAY_BASE_URL=https://gateway.resonantia.me \
npm run tauri build
```

Outputs installers to `src-tauri/target/release/bundle/`.

### Gateway (Docker)

```bash
# from resonantia/
docker build -f gateway/Dockerfile -t resonantia-gateway .
```

A `Dockerfile` for the gateway is on the roadmap. Until then:

```bash
cargo build --release --manifest-path gateway/Cargo.toml
./gateway/target/release/resonantia-gateway
```

---

## Key Source Files

| File | What it does |
|---|---|
| `src/lib/Weaver.svelte` | Entire UI state machine — canvas, settings, sync, calibration, compose, telescope |
| `src/lib/cloudAuth.ts` | Clerk JS: sign-in, token fetch, `getCloudAccount()` |
| `src/lib/resonantiaClient.ts` | Routes calls to Tauri `invoke` or web fetch client depending on runtime |
| `packages/resonantia-core/src/client.ts` | TypeScript client interface + Tauri command bridge |
| `packages/resonantia-core/src/webClient.ts` | All web-mode logic — SurrealDB setup, sync, calibration |
| `src-tauri/resonantia-core/src/lib.rs` | Tauri Rust commands + SurrealDB + Ollama + STTP runtime |
| `gateway/src/main.rs` | Axum gateway: auth, tenant routing, account store, REST routes |
| `vite.config.js` | Dev gateway proxy plugin (routes `/__gateway_proxy__` to avoid CORS in browser dev) |

---

## Adding a New Gateway Route

1. Add the handler function in `gateway/src/main.rs`
2. Register it in the `Router::new()` chain in `main()`
3. If it needs account info, call `resolve_tenant_context()` — `user_id` is on the returned `TenantRequestContext`
4. If it needs admin auth, use `check_admin_secret(&headers, secret)`
5. If it's client-facing, add a corresponding method to `ResonantiaClient` in `packages/resonantia-core/src/client.ts`, implement in `webClient.ts`, and wire through `resonantiaClient.ts`

---

## Roadmap (near-term)

- [ ] Gateway Dockerfile
- [ ] Stripe/Polar webhook → `PATCH /api/v1/account/tier` integration
- [ ] Local AI model hookup (Ollama default config)
- [ ] Gateway env docs update (`RESONANTIA_GATEWAY_ADMIN_SECRET`)
- [ ] Tauri token refresh timing (post-sign-in auto-refresh path)
- [ ] Account storage stats in sync popover
