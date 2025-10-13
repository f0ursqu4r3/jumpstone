# Operations Playbook

This playbook captures day-to-day runbook items for the OpenGuild backend. Treat it as a living document—update it whenever we learn something new in production or adjust our deployment strategy.

## Environments

| Environment | Description            | Primary Location | Notes                             |
|-------------|------------------------|------------------|-----------------------------------|
| `dev`       | Shared integration env | TBD (k8s)        | Rolling updates, no paging        |
| `staging`   | Pre-production         | TBD (k8s)        | Mirrors prod config, manual gates |
| `prod`      | Customer-facing        | TBD (k8s)        | Pager duty, change approvals      |

## Deployments

### Standard rollout

1. Merge PRs to `main`; CI must be green (`Rust CI` workflow).
2. Cut a release tag (e.g., `v0.5.0`) and publish container images:
   ```bash
   cargo xtask ci          # local validation
   docker build -t registry/openguild-server:v0.5.0 backend
   docker push registry/openguild-server:v0.5.0
   ```
3. Apply manifests (example using kustomize/helm TBD):
   ```bash
   kubectl apply -k deploy/k8s/overlays/staging
   ```
4. Wait for rollout:
   ```bash
   kubectl rollout status deploy/openguild-server -n staging
   ```
5. Smoke test:
   ```bash
   curl -s https://staging.api.openguild.dev/ready | jq .
   ```
6. Promote to production (same commands pointing at prod namespace) once staging validates.

### Rollback

1. Identify previous healthy image/tag (`kubectl rollout history deploy/openguild-server -n prod`).
2. Patch deployment:
   ```bash
   kubectl rollout undo deploy/openguild-server -n prod --to-revision=<REV>
   ```
3. Confirm rollback status and monitor logs:
   ```bash
   kubectl rollout status deploy/openguild-server -n prod
   kubectl logs deploy/openguild-server -n prod -f
   ```
4. Capture a postmortem entry (see Incident Response) and update this playbook if new mitigations arise.

## Observability & Monitoring

- **Metrics** — Scraped by Prometheus (see `docs/OBSERVABILITY.md`). Key alerts:
  - `openguild_http_request_duration_seconds` p95 > 750 ms for 5m.
  - `openguild_messaging_events_total{outcome="dropped"}` > 0 for 5m.
  - `openguild_websocket_queue_depth` > 196 for 3m (per channel).
- **Logs** — All http request spans include `request_id`; ensure log aggregation retains this field. Use it to correlate across services.
- **Dashboards** — Grafana “OpenGuild Overview” dashboard (auto-provisioned); clone per-environment dashboards as needed.
- **Synthetic Checks** — TODO: Add health/ready/liveness probes + optional pingdom style external monitors (track in backlog).

### Adding new metrics

1. Extend `MetricsContext` (backend/crates/server/src/metrics.rs).
2. Update handlers to record metrics.
3. Add scraping rules or dashboard panels as needed.
4. Update alert thresholds when promoting to production.

## HTTP Security Headers

- The gateway injects baseline headers on every response:  
  `Content-Security-Policy: default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`  
  `Referrer-Policy: no-referrer`  
  `X-Content-Type-Options: nosniff`  
  `X-Frame-Options: DENY`
- These defaults live in `build_app` (`backend/crates/server/src/main.rs`). Adjustments require a code change and security review; avoid ad-hoc overrides without re-running the threat model.
- Confirm downstream proxies/CDNs preserve the headers. If a proxy rewrites responses, make sure it re-applies the same policy set.

## Incident Response

1. **Triage**
   - Acknowledge alert/page (PagerDuty/Slack).
   - Check Grafana dashboards for anomalies.
   - Review recent deploys (`git log` / #deployments channel).
2. **Stabilise**
   - Mitigate (rollback, scale, feature flag) within 10 minutes.
   - Capture request IDs / logs for failing transactions.
3. **Communicate**
   - Post status updates in `#ops` every 15 minutes while active.
   - Open a status page incident if customer impact > 5 minutes.
4. **Post-Incident**
   - File a retro issue including: timeline, blast radius, contributing factors, action items.
   - Update dashboards/alerts/playbooks with learnings.

### Common Runbooks

| Scenario                        | Action |
|---------------------------------|--------|
| High HTTP latency alert        | Check `openguild_http_request_duration_seconds` panel → inspect upstream dependencies (DB, NATS). |
| Messaging drops > threshold    | Verify queue depth, inspect storage availability, and check NATS health. |
| WebSocket queue depth > 196    | Identify offending channel → consider scaling server replicas, trim replay window, or investigate clients. |
| Auth failures spike            | Inspect Postgres connectivity and `UserRepository::verify_credentials` logs. Fall back to in-memory auth only if explicitly approved. |

## Change Management

- **Pre-deploy checklist**
  - CI green (`cargo xtask ci` locally).
  - Database migrations reviewed and applied in staging.
  - Observability dashboards updated with new metrics.
- **Post-deploy validation**
  - `/health` and `/ready` endpoints succeed.
  - Key metrics within SLO bounds for 15 minutes.
  - No new error logs with `level>=ERROR`.

## Contact & Ownership

- **Primary owner**: Backend team (`#backend-platform`).
- **Escalation path**: Backend → SRE → Incident Commander.
- **Documentation owners**: Update `docs/OBSERVABILITY.md` and this playbook when workflows change.

---

_Revision history_

| Date       | Author | Notes                                 |
|------------|--------|---------------------------------------|
| 2025-10-13 | Team   | Initial draft (observability + ops)   |
