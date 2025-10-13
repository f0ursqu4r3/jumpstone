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

The compose file enables the server's dedicated metrics listener at `0.0.0.0:9100`. Prometheus and Grafana will start automatically once the server is healthy.

### Service Endpoints

| Service     | URL                | Credentials |
|-------------|--------------------|-------------|
| Prometheus  | http://localhost:9090 | n/a         |
| Grafana     | http://localhost:3000 | `admin` / `admin` (change on first login) |

## Grafana dashboards

Grafana is pre-provisioned with an **OpenGuild Overview** dashboard. It surfaces:

- **HTTP Request Duration (p95)** — based on the `openguild_http_request_duration_seconds` histogram. Track sustained p95 latency > 500 ms.
- **HTTP Requests Per Second** — derived from `openguild_http_requests_total`. Investigate sudden drops to zero or unexpected spikes.
- **Messaging Events** — `openguild_messaging_events_total` by outcome (`delivered`, `no_subscribers`, `dropped`). Alert when `dropped` exceeds 0.1 events/sec for 5 minutes.
- **WebSocket Queue Depth** — `openguild_websocket_queue_depth` gauge per channel. Alert when queue depth stays above 128 for >2 minutes, indicating backpressure risk.

### Creating new dashboards

Add JSON dashboards under `deploy/observability/grafana/provisioning/dashboards/json`. Grafana watches this directory on startup; restarting the `grafana` container applies changes.

## Alerting strategy (initial)

Prometheus is currently running without Alertmanager. To experiment locally:

1. Define alerting rules in `deploy/observability/prometheus.yml`.
2. Restart the Prometheus container (`docker compose restart prometheus`).

Suggested baseline rules:

- p95 latency > 750 ms for 5 minutes (severe).
- WebSocket queue depth > 196 for 3 minutes.
- Messaging `dropped` rate > 0 for 5 minutes.

Upstream environments should integrate Alertmanager + paging (out of scope for this local stack).

## CI considerations

The GitHub Actions workflow (`.github/workflows/rust-ci.yml`) runs `cargo fmt`, `cargo clippy`, and `cargo test`. To add regression coverage for metrics:

- Extend the workflow with a step that hits the `/metrics` endpoint (via `curl`) while the server integration tests run.
- Alternatively, add a smoke test in `xtask` that scrapes Prometheus (pending until Alertmanager is wired).

## Cleaning up

```bash
docker compose down -v
```

This stops all services and clears Prometheus/Grafana state volumes.

