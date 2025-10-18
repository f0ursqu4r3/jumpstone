# Observability Toolkit

This project ships a local observability stack so you can inspect metrics, confirm alert thresholds, and iterate on dashboards while developing.

## Prerequisites

- Docker and Docker Compose v2
- A running `openguild-server` container (see `deploy/docker-compose.yml`)

## Bringing the stack online

```bash
cd deploy
docker compose up -d postgres minio nats server prometheus grafana
```

The compose file enables the server's dedicated metrics listener at `0.0.0.0:9100`. Prometheus and Grafana start automatically once the server is healthy.

### Service endpoints

| Service    | URL                     | Credentials                     |
|------------|-------------------------|---------------------------------|
| Prometheus | http://localhost:9090   | n/a                             |
| Grafana    | http://localhost:3000   | `admin` / `admin` (change ASAP) |

## Grafana dashboards

Grafana is pre-provisioned with an **OpenGuild Overview** dashboard. It surfaces:

- **HTTP Request Duration (p95)** - based on the `openguild_http_request_duration_seconds` histogram. Track sustained p95 latency above 500 ms.
- **HTTP Requests Per Second** - derived from `openguild_http_requests_total`. Investigate sudden drops to zero or unexpected spikes.
- **Messaging Events** - `openguild_messaging_events_total` by outcome (`delivered`, `no_subscribers`, `dropped`). Alert when `dropped` exceeds 0.1 events/sec for 5 minutes.
- **Messaging Rejections** - `openguild_messaging_rejections_total` labelled by reason (`unauthorized`, `guild_name_empty`, `guild_name_length`, `channel_name_empty`, `channel_name_length`, `message_empty`, `message_length`, `sender_mismatch`, `message_rate_limit`, `ip_rate_limit`, `websocket_limit`). Sustained non-zero counts merit investigation.
- **WebSocket Queue Depth** - `openguild_websocket_queue_depth` gauge per channel. Alert when queue depth stays above 128 for more than 2 minutes, indicating backpressure.

### Creating new dashboards

Add JSON dashboards under `deploy/observability/grafana/provisioning/dashboards/json`. Grafana watches this directory on startup; restart the `grafana` container to apply changes.

## Request ID usage guidelines

- The server injects `X-Request-Id` when clients omit it; downstream services should forward the header intact.
- Log forwarders (Vector, Fluent Bit, etc.) must retain the structured `request_id` field when shipping JSON logs. If parsing text logs, promote `request_id` to a top-level attribute.
- Incident responders can pivot on `request_id=<uuid>` inside Grafana Explore or the log aggregation UI to correlate requests across systems.
- Client SDKs and frontend code should generate deterministic request IDs when possible so traces span multiple services.

## Promoting observability beyond local dev

- **Staging** - Deploy Prometheus and Grafana alongside the application stack. Reuse `deploy/observability/prometheus.yml` as a base and override scrape targets via Helm/Kustomize. Inject `OPENGUILD_SERVER__SERVER_NAME` per environment so dashboards label events with the correct origin.
- **CI smoke** - Add a pipeline step that runs `cargo xtask test --features metrics` and curls `/metrics` to catch exporter regressions before merge.
- **Alertmanager** - Connect Prometheus to a managed Alertmanager (PagerDuty/Slack) before promoting to production. Store receiver secrets in your secrets manager and mount them into the Prometheus deployment.

Document environment-specific URLs and credentials in `docs/OPERATIONS.md` once staging is live.

## Alerting strategy

For local validation without Alertmanager:

1. Define alerting rules in `deploy/observability/prometheus.yml`.
2. Restart Prometheus (`docker compose restart prometheus`).

Recommended thresholds:

- `openguild_http_request_duration_seconds` p95 > 750 ms for 5 minutes (page ops).
- `openguild_websocket_queue_depth` > 196 for 3 minutes per channel (warn).
- `openguild_messaging_events_total{outcome="dropped"}` > 0 for 5 minutes (warn).
- `openguild_messaging_rejections_total{reason!="websocket_limit"}` rate > 0 for 1 minute (per reason). Escalate immediately for `unauthorized`, `sender_mismatch`, or `ip_rate_limit`.
- `openguild_messaging_rejections_total{reason="websocket_limit"}` rate > 0 for 10 minutes (capacity tuning).
- `openguild_messaging_rejections_total{reason="message_rate_limit"}` rate > 5 for 15 minutes (possible abuse).

When Alertmanager is wired up, mirror these rules and map severities to PagerDuty (page) or Slack/email (warn) receivers.

## CI considerations

The GitHub Actions workflow (`.github/workflows/rust-ci.yml`) runs `cargo fmt`, `cargo clippy`, and `cargo test`. To add regression coverage for metrics:

- Extend the workflow with a step that hits the `/metrics` endpoint (via `curl`) while the server integration tests run.
- Alternatively, add a smoke test in `xtask` that scrapes Prometheus once Alertmanager is part of the staging stack.

## Cleaning up

```bash
docker compose down -v
```

This stops all services and clears Prometheus/Grafana volumes.
