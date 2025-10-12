#![cfg(feature = "metrics")]

use anyhow::Result;
use prometheus::{Encoder, IntCounterVec, Opts, Registry, TextEncoder};
use std::sync::Arc;

#[derive(Clone)]
pub struct MetricsContext {
    registry: Registry,
    pub http_requests_total: IntCounterVec,
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

        Ok(Arc::new(Self {
            registry,
            http_requests_total: counter,
        }))
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        TextEncoder::new().encode(&self.registry.gather(), &mut buffer)?;
        Ok(buffer)
    }
}
