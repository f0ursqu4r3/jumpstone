# Operations Playbook

This playbook captures day-to-day runbook items for the OpenGuild backend. Treat it as a living document and update it whenever we learn something new in production or adjust our deployment strategy.

## Environments

| Environment | Description            | Primary Location | Notes                             |
| ----------- | ---------------------- | ---------------- | --------------------------------- |
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

> Ensure each manifest sets `OPENGUILD_SERVER__SERVER_NAME` (for example `dev.openguild.local`, `staging.api.openguild.dev`, `api.openguild.com`) so tokens and events advertise the correct homeserver. The local Docker Compose stack ships with `dev.openguild.local` as a reference value.

### Rollback

1. Identify the previous healthy image/tag:

   ```bash
   kubectl rollout history deploy/openguild-server -n prod
   ```

2. Patch the deployment:

   ```bash
   kubectl rollout undo deploy/openguild-server -n prod --to-revision=<REV>
   ```

3. Confirm rollback status and monitor logs:

   ```bash
   kubectl rollout status deploy/openguild-server -n prod
   kubectl logs deploy/openguild-server -n prod -f
   ```

4. Capture a postmortem entry (see Incident Response) and update this playbook if new mitigations arise.

## Runtime configuration invariants

- Track environment-specific overrides (Helm values, Terraform variables, etc.) in source control. Document who owns each override.
- `OPENGUILD_SERVER__SERVER_NAME` must map to the canonical homeserver name per environment. Example values:
  - `dev.openguild.local`
  - `staging.api.openguild.dev`
  - `api.openguild.com`
- Record staging and production Grafana/Prometheus endpoints in this document once provisioned.

## Observability & Monitoring

- **Metrics** - Scraped by Prometheus (see `docs/OBSERVABILITY.md`). Key alerts:
  - `openguild_http_request_duration_seconds` p95 > 750 ms for 5 minutes.
  - `openguild_messaging_events_total{outcome="dropped"}` > 0 for 5 minutes.
  - `openguild_websocket_queue_depth` > 196 for 3 minutes (per channel).
- **Logs** - All HTTP spans include `request_id`. Configure Vector, Fluent Bit, or your log shipper of choice to retain the structured `request_id` field so on-call engineers can pivot on it in Grafana Explore or the central log UI.
- **Dashboards** - Grafana ships with the "OpenGuild Overview" dashboard; clone per-environment dashboards as needed.
- **Synthetic checks** - TODO: Add health/ready/liveness probes plus optional external monitors (track in backlog).

### Adding new metrics

1. Extend `MetricsContext` (`backend/crates/server/src/metrics.rs`).
2. Update handlers to record metrics.
3. Add scraping rules or dashboard panels as needed.
4. Update alert thresholds when promoting to production.

## HTTP security headers

- The gateway injects baseline headers on every response:
  - `Content-Security-Policy: default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'`
  - `Referrer-Policy: no-referrer`
  - `X-Content-Type-Options: nosniff`
  - `X-Frame-Options: DENY`
- These defaults live in `build_app` (`backend/crates/server/src/main.rs`). Adjustments require a code change and security review; avoid ad-hoc overrides without re-running the threat model.
- Confirm downstream proxies and CDNs preserve the headers. If a proxy rewrites responses, ensure it re-applies the same policy set.

## Incident response

1. **Triage**
   - Acknowledge alert/page (PagerDuty/Slack).
   - Check Grafana dashboards for anomalies.
   - Review recent deploys (`git log` or #deployments channel).
2. **Stabilise**
   - Mitigate (rollback, scale, feature flag) within 10 minutes.
   - Capture request IDs and logs for failing transactions.
3. **Communicate**
   - Post status updates in `#ops` every 15 minutes while active.
   - Open a status page incident if customer impact exceeds 5 minutes.
4. **Post-incident**
   - File a retro issue including timeline, blast radius, contributing factors, and action items.
   - Update dashboards, alerts, and this playbook with new learnings.

### Common runbooks

| Scenario                    | Action                                                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| High HTTP latency alert     | Check `openguild_http_request_duration_seconds` panels and inspect upstream dependencies (DB, NATS).                                  |
| Messaging drops above limit | Verify queue depth, inspect storage availability, and check NATS health.                                                              |
| WebSocket queue depth > 196 | Identify the offending channel, consider scaling server replicas, trim the replay window, or investigate client behaviour.            |
| Auth failures spike         | Inspect Postgres connectivity and `UserRepository::verify_credentials` logs. Fall back to in-memory auth only if explicitly approved. |

## Change management

- **Pre-deploy checklist**
  - CI green (`cargo xtask ci` locally).
  - Database migrations reviewed and applied in staging.
  - Observability dashboards updated with new metrics.
- **Post-deploy validation**
  - `/health` and `/ready` endpoints succeed.
  - Key metrics remain within SLO bounds for 15 minutes.
  - No new error logs with `level>=ERROR`.

## Contact & ownership

- **Primary owner**: Backend team (`#backend-platform`).
- **Escalation path**: Backend -> SRE -> Incident Commander.
- **Documentation owners**: Update `docs/OBSERVABILITY.md` and this playbook when workflows change.

---

_Revision history_

| Date       | Author | Notes                                       |
| ---------- | ------ | ------------------------------------------- |
| 2025-10-14 | Team   | Added server_name invariant + log guidance  |
| 2025-10-13 | Team   | Initial draft (observability + ops)         |
