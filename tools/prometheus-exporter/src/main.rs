#![allow(clippy::new_without_default)]
use anyhow::{Context, Result};
use clap::Parser;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server, StatusCode};
use prometheus::{Counter, Encoder, Gauge, Histogram, Registry, TextEncoder};
use soroban_sas_sdk::events::{parse_contract_event, SasEvent};
use soroban_sdk::xdr::{ScVal, TransactionMeta, XdrDeserialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Prometheus metrics exporter for Soroban SAS events
///
/// Subscribes to Soroban network events and exports Prometheus metrics
/// for monitoring SAS contract activity.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Soroban RPC URL
    #[arg(long, env = "SOROBAN_RPC_URL", default_value = "http://localhost:8000/soroban/rpc")]
    rpc_url: String,

    /// SAS contract ID to monitor
    #[arg(long, env = "SAS_CONTRACT_ID")]
    sas_contract_id: String,

    /// Schema Registry contract ID to monitor
    #[arg(long, env = "SCHEMA_REGISTRY_CONTRACT_ID")]
    schema_registry_contract_id: Option<String>,

    /// Indexer contract ID to monitor
    #[arg(long, env = "INDEXER_CONTRACT_ID")]
    indexer_contract_id: Option<String>,

    /// Prometheus metrics server address
    #[arg(long, env = "METRICS_ADDR", default_value = "0.0.0.0:9090")]
    metrics_addr: SocketAddr,

    /// Polling interval for events (in seconds)
    #[arg(long, env = "POLL_INTERVAL", default_value = "5")]
    poll_interval: u64,
}

/// Metrics registry containing all SAS event metrics
#[derive(Clone)]
struct SasMetrics {
    registry: Registry,
    
    // Event counters
    attestation_issued_total: Counter,
    attestation_revoked_total: Counter,
    attestation_renewed_total: Counter,
    batch_attested_total: Counter,
    batch_revoked_total: Counter,
    schema_registered_total: Counter,
    schema_deprecated_total: Counter,
    attester_key_registered_total: Counter,
    attester_key_rotated_total: Counter,
    attester_key_revoked_total: Counter,
    indexer_updated_total: Counter,
    indexer_strict_updated_total: Counter,
    contract_upgraded_total: Counter,
    contract_paused_total: Counter,
    contract_unpaused_total: Counter,
    admin_transfer_proposed_total: Counter,
    admin_transfer_completed_total: Counter,
    schema_delegate_added_total: Counter,
    schema_delegate_removed_total: Counter,
    schema_ownership_transferred_total: Counter,
    fee_config_updated_total: Counter,
    indexing_progress_total: Counter,

    // Gauges for current state
    active_attestations: Gauge,
    active_schemas: Gauge,
    registered_attester_keys: Gauge,

    // Histogram for processing times
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

        let attestation_renewed_total = Counter::new(
            "sas_attestation_renewed_total",
            "Total number of attestations renewed",
        )?;
        registry.register(Box::new(attestation_renewed_total.clone()))?;

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

        let schema_deprecated_total = Counter::new(
            "sas_schema_deprecated_total",
            "Total number of schemas deprecated",
        )?;
        registry.register(Box::new(schema_deprecated_total.clone()))?;

        let attester_key_registered_total = Counter::new(
            "sas_attester_key_registered_total",
            "Total number of attester keys registered",
        )?;
        registry.register(Box::new(attester_key_registered_total.clone()))?;

        let attester_key_rotated_total = Counter::new(
            "sas_attester_key_rotated_total",
            "Total number of attester keys rotated",
        )?;
        registry.register(Box::new(attester_key_rotated_total.clone()))?;

        let attester_key_revoked_total = Counter::new(
            "sas_attester_key_revoked_total",
            "Total number of attester keys revoked",
        )?;
        registry.register(Box::new(attester_key_revoked_total.clone()))?;

        let indexer_updated_total = Counter::new(
            "sas_indexer_updated_total",
            "Total number of indexer updates",
        )?;
        registry.register(Box::new(indexer_updated_total.clone()))?;

        let indexer_strict_updated_total = Counter::new(
            "sas_indexer_strict_updated_total",
            "Total number of indexer strict policy updates",
        )?;
        registry.register(Box::new(indexer_strict_updated_total.clone()))?;

        let contract_upgraded_total = Counter::new(
            "sas_contract_upgraded_total",
            "Total number of contract upgrades",
        )?;
        registry.register(Box::new(contract_upgraded_total.clone()))?;

        let contract_paused_total = Counter::new(
            "sas_contract_paused_total",
            "Total number of contract pause events",
        )?;
        registry.register(Box::new(contract_paused_total.clone()))?;

        let contract_unpaused_total = Counter::new(
            "sas_contract_unpaused_total",
            "Total number of contract unpause events",
        )?;
        registry.register(Box::new(contract_unpaused_total.clone()))?;

        let admin_transfer_proposed_total = Counter::new(
            "sas_admin_transfer_proposed_total",
            "Total number of admin transfer proposals",
        )?;
        registry.register(Box::new(admin_transfer_proposed_total.clone()))?;

        let admin_transfer_completed_total = Counter::new(
            "sas_admin_transfer_completed_total",
            "Total number of admin transfer completions",
        )?;
        registry.register(Box::new(admin_transfer_completed_total.clone()))?;

        let schema_delegate_added_total = Counter::new(
            "sas_schema_delegate_added_total",
            "Total number of schema delegates added",
        )?;
        registry.register(Box::new(schema_delegate_added_total.clone()))?;

        let schema_delegate_removed_total = Counter::new(
            "sas_schema_delegate_removed_total",
            "Total number of schema delegates removed",
        )?;
        registry.register(Box::new(schema_delegate_removed_total.clone()))?;

        let schema_ownership_transferred_total = Counter::new(
            "sas_schema_ownership_transferred_total",
            "Total number of schema ownership transfers",
        )?;
        registry.register(Box::new(schema_ownership_transferred_total.clone()))?;

        let fee_config_updated_total = Counter::new(
            "sas_fee_config_updated_total",
            "Total number of fee configuration updates",
        )?;
        registry.register(Box::new(fee_config_updated_total.clone()))?;

        let indexing_progress_total = Counter::new(
            "sas_indexing_progress_total",
            "Total number of indexing progress events",
        )?;
        registry.register(Box::new(indexing_progress_total.clone()))?;

        let active_attestations = Gauge::new(
            "sas_active_attestations",
            "Current number of active attestations",
        )?;
        registry.register(Box::new(active_attestations.clone()))?;

        let active_schemas = Gauge::new(
            "sas_active_schemas",
            "Current number of active schemas",
        )?;
        registry.register(Box::new(active_schemas.clone()))?;

        let registered_attester_keys = Gauge::new(
            "sas_registered_attester_keys",
            "Current number of registered attester keys",
        )?;
        registry.register(Box::new(registered_attester_keys.clone()))?;

        let event_processing_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "sas_event_processing_duration_seconds",
                "Duration of event processing in seconds",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
        )?;
        registry.register(Box::new(event_processing_duration.clone()))?;

        Ok(Self {
            registry,
            attestation_issued_total,
            attestation_revoked_total,
            attestation_renewed_total,
            batch_attested_total,
            batch_revoked_total,
            schema_registered_total,
            schema_deprecated_total,
            attester_key_registered_total,
            attester_key_rotated_total,
            attester_key_revoked_total,
            indexer_updated_total,
            indexer_strict_updated_total,
            contract_upgraded_total,
            contract_paused_total,
            contract_unpaused_total,
            admin_transfer_proposed_total,
            admin_transfer_completed_total,
            schema_delegate_added_total,
            schema_delegate_removed_total,
            schema_ownership_transferred_total,
            fee_config_updated_total,
            indexing_progress_total,
            active_attestations,
            active_schemas,
            registered_attester_keys,
            event_processing_duration,
        })
    }

    fn record_event(&self, event: &SasEvent, contract_id: &str) {
        let timer = self.event_processing_duration.start_timer();

        match event {
            SasEvent::AttestationIssued(_) => {
                self.attestation_issued_total
                    .inc();
                self.active_attestations.inc();
            }
            SasEvent::AttestationRevoked(_) => {
                self.attestation_revoked_total
                    .inc();
                self.active_attestations.dec();
            }
            SasEvent::AttestationRenewed(_) => {
                self.attestation_renewed_total.inc();
            }
            SasEvent::BatchAttested(batch) => {
                self.batch_attested_total.inc();
                self.active_attestations
                    .inc_by(batch.count as f64);
            }
            SasEvent::BatchRevoked(batch) => {
                self.batch_revoked_total.inc();
                self.active_attestations
                    .dec_by(batch.count as f64);
            }
            SasEvent::SchemaRegistered(_) => {
                self.schema_registered_total.inc();
                self.active_schemas.inc();
            }
            SasEvent::SchemaDeprecated(_) => {
                self.schema_deprecated_total.inc();
                self.active_schemas.dec();
            }
            SasEvent::AttesterKeyRegistered(_) => {
                self.attester_key_registered_total.inc();
                self.registered_attester_keys.inc();
            }
            SasEvent::AttesterKeyRotated(_) => {
                self.attester_key_rotated_total.inc();
            }
            SasEvent::AttesterKeyRevoked(_) => {
                self.attester_key_revoked_total.inc();
                self.registered_attester_keys.dec();
            }
            SasEvent::IndexerUpdated(_) => {
                self.indexer_updated_total.inc();
            }
            SasEvent::IndexerStrictUpdated(_) => {
                self.indexer_strict_updated_total.inc();
            }
            SasEvent::ContractUpgraded(_) => {
                self.contract_upgraded_total.inc();
            }
            SasEvent::ContractPaused(_) => {
                self.contract_paused_total.inc();
            }
            SasEvent::ContractUnpaused(_) => {
                self.contract_unpaused_total.inc();
            }
            SasEvent::AdminTransferProposed(_) => {
                self.admin_transfer_proposed_total.inc();
            }
            SasEvent::AdminTransferCompleted(_) => {
                self.admin_transfer_completed_total.inc();
            }
            SasEvent::SchemaDelegateAdded(_) => {
                self.schema_delegate_added_total.inc();
            }
            SasEvent::SchemaDelegateRemoved(_) => {
                self.schema_delegate_removed_total.inc();
            }
            SasEvent::SchemaOwnershipTransferred(_) => {
                self.schema_ownership_transferred_total.inc();
            }
            SasEvent::FeeConfigUpdated(_) => {
                self.fee_config_updated_total.inc();
            }
            SasEvent::IndexingProgress(_) => {
                self.indexing_progress_total.inc();
            }
        }

        timer.observe_duration();
    }
}

/// State for tracking the latest processed ledger
#[derive(Clone)]
struct ExporterState {
    latest_ledger: Arc<RwLock<u32>>,
    metrics: Arc<SasMetrics>,
    contract_ids: Arc<Vec<String>>,
}

async fn metrics_handler(state: ExporterState) -> Result<Response<Body>> {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .context("Failed to encode metrics")?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", encoder.format_type())
        .body(Body::from(buffer))
        .context("Failed to build response")?)
}

async fn poll_events(rpc_url: &str, state: &ExporterState) -> Result<()> {
    let mut latest_ledger = state.latest_ledger.write().await;
    
    // Get the latest ledger sequence via RPC
    let client = reqwest::Client::new();
    let latest_response = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestLedger"
        }))
        .send()
        .await
        .context("Failed to get latest ledger")?;
    
    let latest_json: serde_json::Value = latest_response
        .json()
        .await
        .context("Failed to parse latest ledger response")?;
    
    let current_sequence = latest_json["result"]["sequence"]
        .as_u64()
        .context("Failed to extract sequence number")? as u32;
    
    // If this is the first poll, start from the current ledger
    if *latest_ledger == 0 {
        *latest_ledger = current_sequence;
        info!("Starting event polling from ledger {}", current_sequence);
        return Ok(());
    }
    
    // Poll for events from the next ledger
    let start_ledger = *latest_ledger + 1;
    
    if start_ledger > current_sequence {
        // No new ledgers
        return Ok(());
    }
    
    // Get events for the ledger range via RPC
    let events_response = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getEvents",
            "params": {
                "startLedger": start_ledger,
                "filter": {
                    "topics": [[]]
                }
            }
        }))
        .send()
        .await
        .context("Failed to get events")?;
    
    let events_json: serde_json::Value = events_response
        .json()
        .await
        .context("Failed to parse events response")?;
    
    // Process events
    if let Some(events) = events_json["result"]["events"].as_array() {
        for event in events {
            if let Some(contract_id) = event["contractId"].as_str() {
                // Check if this event is from a monitored contract
                if state.contract_ids.contains(&contract_id.to_string()) {
                    // Parse the event body from XDR
                    if let Some(event_body_xdr) = event["body"]["value"]["xdr_base64"].as_str() {
                        // Decode XDR and parse event
                        if let Ok(decoded_bytes) = base64::decode(event_body_xdr) {
                            if let Ok(sc_val) = ScVal::from_xdr(&decoded_bytes) {
                                if let Ok(parsed) = parse_contract_event(&sc_val) {
                                    state.metrics.record_event(&parsed, contract_id);
                                    info!(
                                        "Recorded event from contract {}: {:?}",
                                        contract_id, parsed
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Update the latest processed ledger
    *latest_ledger = current_sequence;
    
    Ok(())
}

async fn run_event_loop(rpc_url: String, state: ExporterState, interval: Duration) {
    let mut timer = tokio::time::interval(interval);
    
    loop {
        timer.tick().await;
        
        if let Err(e) = poll_events(&rpc_url, &state).await {
            error!("Error polling events: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();
    
    info!("Starting SAS Prometheus metrics exporter");
    info!("RPC URL: {}", args.rpc_url);
    info!("SAS Contract ID: {}", args.sas_contract_id);
    info!("Metrics address: {}", args.metrics_addr);
    
    // Build list of contract IDs to monitor
    let mut contract_ids = vec![args.sas_contract_id.clone()];
    if let Some(ref schema_id) = args.schema_registry_contract_id {
        contract_ids.push(schema_id.clone());
    }
    if let Some(ref indexer_id) = args.indexer_contract_id {
        contract_ids.push(indexer_id.clone());
    }
    
    // Initialize metrics
    let metrics = Arc::new(SasMetrics::new()?);
    
    // Initialize state
    let state = ExporterState {
        latest_ledger: Arc::new(RwLock::new(0)),
        metrics,
        contract_ids: Arc::new(contract_ids),
    };
    
    // Spawn event polling task
    let poll_state = state.clone();
    let rpc_url = args.rpc_url.clone();
    let interval = Duration::from_secs(args.poll_interval);
    tokio::spawn(async move {
        run_event_loop(rpc_url, poll_state, interval).await;
    });
    
    // Start metrics server
    let state_clone = state.clone();
    let make_svc = make_service_fn(move |_conn| {
        let state = state_clone.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |_req| {
                let state = state.clone();
                async move { metrics_handler(state).await.map_err(|e| hyper::Error::msg(e.to_string())) }
            }))
        }
    });
    
    let server = Server::bind(&args.metrics_addr).serve(make_svc);
    
    info!("Metrics server listening on {}", args.metrics_addr);
    
    server.await.context("Server error")?;

    Ok(())
}

#[cfg(test)]
mod tests;
