# SAS Prometheus Metrics Exporter

A Prometheus metrics exporter for Soroban SAS (Stellar Attestation Service) events.

## Overview

This tool subscribes to Soroban network events from SAS contracts and exports Prometheus metrics for monitoring contract activity. It tracks all SAS event types including attestations, schema registrations, key management, and administrative operations.

## Metrics

### Counters

- `sas_attestation_issued_total` - Total number of attestations issued
- `sas_attestation_revoked_total` - Total number of attestations revoked
- `sas_attestation_renewed_total` - Total number of attestations renewed
- `sas_batch_attested_total` - Total number of batch attestations completed
- `sas_batch_revoked_total` - Total number of batch revocations completed
- `sas_schema_registered_total` - Total number of schemas registered
- `sas_schema_deprecated_total` - Total number of schemas deprecated
- `sas_attester_key_registered_total` - Total number of attester keys registered
- `sas_attester_key_rotated_total` - Total number of attester keys rotated
- `sas_attester_key_revoked_total` - Total number of attester keys revoked
- `sas_indexer_updated_total` - Total number of indexer updates
- `sas_indexer_strict_updated_total` - Total number of indexer strict policy updates
- `sas_contract_upgraded_total` - Total number of contract upgrades
- `sas_contract_paused_total` - Total number of contract pause events
- `sas_contract_unpaused_total` - Total number of contract unpause events
- `sas_admin_transfer_proposed_total` - Total number of admin transfer proposals
- `sas_admin_transfer_completed_total` - Total number of admin transfer completions
- `sas_schema_delegate_added_total` - Total number of schema delegates added
- `sas_schema_delegate_removed_total` - Total number of schema delegates removed
- `sas_schema_ownership_transferred_total` - Total number of schema ownership transfers
- `sas_fee_config_updated_total` - Total number of fee configuration updates
- `sas_indexing_progress_total` - Total number of indexing progress events

### Gauges

- `sas_active_attestations` - Current number of active attestations
- `sas_active_schemas` - Current number of active schemas
- `sas_registered_attester_keys` - Current number of registered attester keys

### Histograms

- `sas_event_processing_duration_seconds` - Duration of event processing in seconds

## Usage

### Build

```bash
cd tools/prometheus-exporter
cargo build --release
```

### Run

```bash
# Basic usage with required SAS contract ID
cargo run -- --sas-contract-id <CONTRACT_ID> --rpc-url <RPC_URL>

# With schema registry and indexer monitoring
cargo run -- \
  --sas-contract-id <SAS_CONTRACT_ID> \
  --schema-registry-contract-id <REGISTRY_CONTRACT_ID> \
  --indexer-contract-id <INDEXER_CONTRACT_ID> \
  --rpc-url <RPC_URL>

# With custom metrics address and poll interval
cargo run -- \
  --sas-contract-id <CONTRACT_ID> \
  --rpc-url <RPC_URL> \
  --metrics-addr 0.0.0.0:9090 \
  --poll-interval 10
```

### Environment Variables

All options can be set via environment variables:

- `SOROBAN_RPC_URL` - Soroban RPC URL (default: `http://localhost:8000/soroban/rpc`)
- `SAS_CONTRACT_ID` - SAS contract ID to monitor (required)
- `SCHEMA_REGISTRY_CONTRACT_ID` - Schema Registry contract ID to monitor (optional)
- `INDEXER_CONTRACT_ID` - Indexer contract ID to monitor (optional)
- `METRICS_ADDR` - Prometheus metrics server address (default: `0.0.0.0:9090`)
- `POLL_INTERVAL` - Polling interval for events in seconds (default: `5`)

### Accessing Metrics

Once running, metrics are available at `http://localhost:9090/metrics`:

```bash
curl http://localhost:9090/metrics
```

## Prometheus Configuration

Add the following to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'soroban-sas'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
```

## Example Queries

### Rate of attestations issued per hour
```
rate(sas_attestation_issued_total[1h])
```

### Active attestations over time
```
sas_active_attestations
```

### Recent schema registrations
```
rate(sas_schema_registered_total[5m])
```

### Batch attestation sizes
```
rate(sas_batch_attested_total[1h]) / rate(sas_attestation_issued_total[1h])
```

## Architecture

The exporter:
1. Polls the Soroban RPC for events at a configurable interval
2. Filters events from monitored SAS contracts
3. Parses events using the `soroban-sas-sdk` event parser
4. Updates Prometheus metrics based on event types
5. Exposes metrics via an HTTP endpoint on `/metrics`

The gauge metrics (`active_attestations`, `active_schemas`, `registered_attester_keys`) are maintained by incrementing on creation/registration events and decrementing on revocation/deprecation events.

## Development

### Run tests

```bash
cargo test
```

### Run with debug logging

```bash
RUST_LOG=debug cargo run -- --sas-contract-id <CONTRACT_ID> --rpc-url <RPC_URL>
```

## License

Same as the parent soroban-sas project.
