#![allow(dead_code)]
#![allow(clippy::new_without_default)]
use anyhow::Result;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Response, Server, StatusCode};
use prometheus::{Counter, Encoder, Gauge, Histogram, Registry, TextEncoder};
use soroban_sas_sdk::events::SasEvent;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

struct Config {
    sas_contract_id: String,
    schema_registry_contract_id: Option<String>,
    indexer_contract_id: Option<String>,
    metrics_addr: SocketAddr,
}

impl Config {
    fn from_env() -> Result<Self> {
        let sas_contract_id = env::var("SAS_CONTRACT_ID")
            .map_err(|_| anyhow::anyhow!("SAS_CONTRACT_ID environment variable is required"))?;

        let schema_registry_contract_id = env::var("SCHEMA_REGISTRY_CONTRACT_ID").ok();
        let indexer_contract_id = env::var("INDEXER_CONTRACT_ID").ok();

        let metrics_addr: SocketAddr = env::var("METRICS_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid METRICS_ADDR: {}", e))?;

        Ok(Self {
            sas_contract_id,
            schema_registry_contract_id,
            indexer_contract_id,
            metrics_addr,
        })
    }
}

#[derive(Clone)]
struct SasMetrics {
    registry: Registry,
    attestation_issued_total: Counter,
    attestation_revoked_total: Counter,
    batch_attested_total: Counter,
    batch_revoked_total: Counter,
    schema_registered_total: Counter,
    fee_config_updated_total: Counter,
    indexer_strict_updated_total: Counter,
    active_attestations: Gauge,
    active_schemas: Gauge,
    event_processing_duration: Histogram,
}

impl SasMetrics {
    fn new() -> Result<Self> {
        let registry = Registry::new();

        let attestation_issued_total = Counter::new(
            "sas_attestation_issued_total",
            "Total number of attestations issued",
        )?;
        registry.register(Box::new(attestation_issued_total.clone()))?;

        let attestation_revoked_total = Counter::new(
            "sas_attestation_revoked_total",
            "Total number of attestations revoked",
        )?;
        registry.register(Box::new(attestation_revoked_total.clone()))?;

        let batch_attested_total = Counter::new(
            "sas_batch_attested_total",
            "Total number of batch attestations completed",
        )?;
        registry.register(Box::new(batch_attested_total.clone()))?;

        let batch_revoked_total = Counter::new(
            "sas_batch_revoked_total",
            "Total number of batch revocations completed",
        )?;
        registry.register(Box::new(batch_revoked_total.clone()))?;

        let schema_registered_total = Counter::new(
            "sas_schema_registered_total",
            "Total number of schemas registered",
        )?;
        registry.register(Box::new(schema_registered_total.clone()))?;

        let fee_config_updated_total = Counter::new(
            "sas_fee_config_updated_total",
            "Total number of fee configuration updates",
        )?;
        registry.register(Box::new(fee_config_updated_total.clone()))?;

        let indexer_strict_updated_total = Counter::new(
            "sas_indexer_strict_updated_total",
            "Total number of indexer strict policy updates",
        )?;
        registry.register(Box::new(indexer_strict_updated_total.clone()))?;

        let active_attestations = Gauge::new(
            "sas_active_attestations",
            "Current number of active attestations",
        )?;
        registry.register(Box::new(active_attestations.clone()))?;

        let active_schemas = Gauge::new("sas_active_schemas", "Current number of active schemas")?;
        registry.register(Box::new(active_schemas.clone()))?;

        let event_processing_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "sas_event_processing_duration_seconds",
                "Duration of event processing in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
            ]),
        )?;
        registry.register(Box::new(event_processing_duration.clone()))?;

        Ok(Self {
            registry,
            attestation_issued_total,
            attestation_revoked_total,
            batch_attested_total,
            batch_revoked_total,
            schema_registered_total,
            fee_config_updated_total,
            indexer_strict_updated_total,
            active_attestations,
            active_schemas,
            event_processing_duration,
        })
    }

    fn record_event(&self, event: &SasEvent, _contract_id: &str) {
        let timer = self.event_processing_duration.start_timer();

        match event {
            SasEvent::AttestationIssued(_) => {
                self.attestation_issued_total.inc();
                self.active_attestations.inc();
            }
            SasEvent::AttestationRevoked(_) => {
                self.attestation_revoked_total.inc();
                self.active_attestations.dec();
            }
            SasEvent::BatchAttested(batch) => {
                self.batch_attested_total.inc();
                for _ in 0..batch.count {
                    self.active_attestations.inc();
                }
            }
            SasEvent::BatchRevoked(batch) => {
                self.batch_revoked_total.inc();
                for _ in 0..batch.count {
                    self.active_attestations.dec();
                }
            }
            SasEvent::SchemaRegistered(_) => {
                self.schema_registered_total.inc();
                self.active_schemas.inc();
            }
            SasEvent::FeeConfigUpdated(_) => {
                self.fee_config_updated_total.inc();
            }
            SasEvent::IndexerStrictUpdated(_) => {
                self.indexer_strict_updated_total.inc();
            }
        }

        timer.observe_duration();
    }
}

#[derive(Clone)]
struct ExporterState {
    metrics: Arc<SasMetrics>,
}

async fn metrics_handler(state: ExporterState) -> hyper::Result<Response<Body>> {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", encoder.format_type())
        .body(Body::from(buffer))
        .unwrap())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let config = Config::from_env()?;

    info!("Starting SAS Prometheus metrics exporter");
    info!("SAS Contract ID: {}", config.sas_contract_id);
    info!("Metrics address: {}", config.metrics_addr);

    let metrics = Arc::new(SasMetrics::new()?);

    let state = ExporterState { metrics };

    let state_clone = state.clone();
    let make_svc = make_service_fn(move |_conn| {
        let state = state_clone.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |_req| {
                let state = state.clone();
                async move { metrics_handler(state).await }
            }))
        }
    });

    let server = Server::bind(&config.metrics_addr).serve(make_svc);

    info!("Metrics server listening on {}", config.metrics_addr);

    server
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests;
