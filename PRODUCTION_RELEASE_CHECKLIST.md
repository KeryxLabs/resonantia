# Resonantia Production Release Checklist

Use this as a launch gate. Do not ship until all Blocker and Launch-Critical items are checked.

## Blockers (Must Pass)

- [ ] End-to-end signup -> checkout -> webhook -> tier upgrade flow passes in production-like environment.
- [ ] Stripe uses Price IDs (price_...) for both paid tiers, never Product IDs (prod_...).
- [ ] Checkout, customer portal, and account endpoints return expected success responses under load.
- [ ] Auth mode behavior verified for both managed cloud and BYO (auth-off and auth-on).
- [ ] BYO direct gateway calls bypass managed proxy and work with empty auth token.
- [ ] CORS confirmed for all expected app origins (managed app, account app, local dev if supported).
- [ ] Gateway host clock sync is stable (NTP enabled, no material drift).
- [ ] No runtime panic on first request (tenant lazy init path tested after cold boot).
- [ ] Tenant cache settings configured and validated in production env.
- [ ] Rollback plan tested with last known good image tags.

## Application Functionality

- [ ] Health endpoint returns status ok and expected transport label.
- [ ] Node store and graph endpoints return valid payloads with and without session filters.
- [ ] Store endpoint validates STTP payload and returns stable error shape.
- [ ] Sync now flow uploads new nodes and downloads remote changes correctly.
- [ ] Duplicate detection is working (no ping-pong reupload loops).
- [ ] Managed cloud entitlement checks block free tier on managed gateway as designed.
- [ ] BYO free-tier sync works when gateway is reachable and configured.
- [ ] Account page state transitions are correct for signed-out, free, resonant, soulful users.
- [ ] Customer portal opens only when stripe customer exists.
- [ ] Stripe webhook event handling updates tiers correctly for success/failure/cancel paths.

## Security

- [ ] Secrets are only provided through environment variables (never committed).
- [ ] Stripe secret key and webhook secret are rotated and scoped correctly.
- [ ] Admin secret is non-default and required for tier override endpoint.
- [ ] Auth mode is explicitly set per environment (no accidental off mode in managed prod).
- [ ] JWT issuer, audience, tenant claim values match intended Clerk setup.
- [ ] Token leeway value is conservative (enough for skew, not overly permissive).
- [ ] CORS allowed origins are explicit in production (avoid wildcard unless temporary emergency).
- [ ] TLS is enforced on public endpoints.
- [ ] Request body sizes and timeout behavior are acceptable to prevent abuse.
- [ ] Logs do not leak tokens, secrets, or sensitive customer identifiers.

## Data and Persistence

- [ ] Remote SurrealDB connection and auth verified in production network.
- [ ] Account schema exists and migrations are idempotent.
- [ ] Tenant isolation model validated (no cross-tenant reads/writes).
- [ ] Backup policy defined for account and node data.
- [ ] Restore procedure tested from backup into staging.
- [ ] Data retention policy documented for logs and node history.
- [ ] Tenant cache max size and idle TTL tuned for expected scale.
- [ ] Memory usage observed over soak test to confirm cache eviction works.

## Reliability and Operations

- [ ] Container restart policy enabled and validated.
- [ ] Liveness/readiness checks configured for web and gateway services.
- [ ] Gateway startup logs reviewed for warnings (price id shape, auth config, cache config).
- [ ] Structured logs are centralized and searchable.
- [ ] Alerting configured for 5xx spikes, panic signatures, webhook failures.
- [ ] Stripe webhook delivery failures alert route configured.
- [ ] Rate limiting strategy documented (or explicit decision to defer).
- [ ] Incident response runbook exists for auth outage, Stripe outage, DB outage.
- [ ] SLO targets defined for sync and checkout paths.

## Performance

- [ ] Cold start and first-request latency measured for gateway.
- [ ] Sync throughput tested with realistic dataset sizes.
- [ ] Graph and nodes query performance tested with large tenant histories.
- [ ] Checkout and account endpoints tested under concurrent access.
- [ ] Browser memory and network behavior tested in long sessions.

## Frontend and UX

- [ ] Settings save/load behavior validated for gateway URL and token fields.
- [ ] Managed vs BYO UX states are clear and non-confusing.
- [ ] Error messages are actionable (CORS, auth, checkout, stripe config).
- [ ] Service worker behavior verified after deploy (no stale config regressions).
- [ ] Hard refresh/cache bust process documented for urgent fixes.
- [ ] Mobile and desktop viewport checks completed.

## BYO Gateway Compatibility

- [ ] BYO gateway implementation tested against BYO gateway spec doc.
- [ ] Legacy aliases supported for store/nodes/graph routes.
- [ ] Nodes endpoint works with and without limit/sessionId query params.
- [ ] Error response shape includes top-level error message.
- [ ] Auth-off behavior explicitly tested with no Authorization header.
- [ ] CORS requirements verified from hosted origin and local origin.

## Release Process

- [ ] Release notes include behavior changes and migration notes.
- [ ] Image tags are immutable and recorded for web/gateway.
- [ ] Deployment order defined (gateway first, then web, then smoke tests).
- [ ] Smoke test checklist executed post-deploy.
- [ ] Rollback criteria and trigger thresholds defined.
- [ ] Rollback commands and previous image tags documented.

## Post-Release (24-48 Hours)

- [ ] Monitor 401, 403, 5xx trends and compare to baseline.
- [ ] Monitor Stripe checkout conversion and webhook success rate.
- [ ] Monitor sync success/failure rates and common failure messages.
- [ ] Monitor memory growth and tenant cache eviction logs.
- [ ] Collect top user-reported issues and triage into hotfix queue.

## Known High-Risk Areas To Double-Check

- [ ] Clock sync on gateway host (token verification sensitive).
- [ ] Stripe env values are Price IDs, not Product IDs.
- [ ] BYO CORS configured for exact app origin.
- [ ] Managed auth token refresh does not interfere with BYO no-auth mode.
- [ ] First request after cold boot does not panic.
