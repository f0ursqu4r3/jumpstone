#![cfg(feature = "metrics")]

use anyhow::Result;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
    TextEncoder,
};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct MetricsContext {
    registry: Registry,
    pub http_requests_total: IntCounterVec,
    pub http_request_duration_seconds: HistogramVec,
    pub messaging_events_total: IntCounterVec,
    pub messaging_rejections_total: IntCounterVec,
    pub websocket_queue_depth: IntGaugeVec,
    db_ready: IntGauge,
}

impl MetricsContext {
    pub fn init() -> Result<Arc<Self>> {
        let registry = Registry::new();

        let counter = IntCounterVec::new(
            Opts::new(
                "openguild_http_requests_total",
                "Number of HTTP responses served, labeled by route and status",
            ),
            &["route", "status"],
        )?;
        registry.register(Box::new(counter.clone()))?;

        let histogram = HistogramVec::new(
            HistogramOpts::new(
                "openguild_http_request_duration_seconds",
                "HTTP request latency in seconds, labeled by route and status",
            )
            .buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["route", "status"],
        )?;
        registry.register(Box::new(histogram.clone()))?;

        let messaging_events_total = IntCounterVec::new(
            Opts::new(
                "openguild_messaging_events_total",
                "Count of messaging events processed by outcome (delivered/no_subscribers/dropped)",
            ),
            &["outcome"],
        )?;
        registry.register(Box::new(messaging_events_total.clone()))?;

        let messaging_rejections_total = IntCounterVec::new(
            Opts::new(
                "openguild_messaging_rejections_total",
                "Count of messaging request rejections by reason",
            ),
            &["reason"],
        )?;
        registry.register(Box::new(messaging_rejections_total.clone()))?;

        let websocket_queue_depth = IntGaugeVec::new(
            Opts::new(
                "openguild_websocket_queue_depth",
                "WebSocket broadcast queue depth per channel",
            ),
            &["channel_id"],
        )?;
        registry.register(Box::new(websocket_queue_depth.clone()))?;

        let db_ready = IntGauge::with_opts(Opts::new(
            "openguild_db_ready",
            "Gauge reflecting database readiness (1 ready, 0 otherwise)",
        ))?;
        registry.register(Box::new(db_ready.clone()))?;

        Ok(Arc::new(Self {
            registry,
            http_requests_total: counter,
            http_request_duration_seconds: histogram,
            messaging_events_total,
            messaging_rejections_total,
            websocket_queue_depth,
            db_ready,
        }))
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        TextEncoder::new().encode(&self.registry.gather(), &mut buffer)?;
        Ok(buffer)
    }

    pub fn set_db_ready(&self, ready: bool) {
        self.db_ready.set(if ready { 1 } else { 0 });
    }

    pub fn observe_http_latency(&self, route: &str, status: u16, latency: Duration) {
        let status_str = status.to_string();
        self.http_request_duration_seconds
            .with_label_values(&[route, status_str.as_str()])
            .observe(latency.as_secs_f64());
    }

    pub fn increment_messaging_events(&self, outcome: &str) {
        self.messaging_events_total
            .with_label_values(&[outcome])
            .inc();
    }

    pub fn increment_messaging_rejection(&self, reason: &str) {
        self.messaging_rejections_total
            .with_label_values(&[reason])
            .inc();
    }

    pub fn set_websocket_queue_depth(&self, channel_id: &str, depth: usize) {
        self.websocket_queue_depth
            .with_label_values(&[channel_id])
            .set(depth as i64);
    }
}
