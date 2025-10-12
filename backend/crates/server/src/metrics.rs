#![cfg(feature = "metrics")]

use anyhow::Result;
use prometheus::{Encoder, IntCounterVec, IntGauge, Opts, Registry, TextEncoder};
use std::sync::Arc;

#[derive(Clone)]
pub struct MetricsContext {
    registry: Registry,
    pub http_requests_total: IntCounterVec,
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

        let db_ready = IntGauge::with_opts(Opts::new(
            "openguild_db_ready",
            "Gauge reflecting database readiness (1 ready, 0 otherwise)",
        ))?;
        registry.register(Box::new(db_ready.clone()))?;

        Ok(Arc::new(Self {
            registry,
            http_requests_total: counter,
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
}
