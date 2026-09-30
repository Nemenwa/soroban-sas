use super::*;
use prometheus::{Counter, Gauge, Histogram, Registry};
use soroban_sas_sdk::events::{
    AttestationIssuedEvent, AttestationRevokedEvent, BatchAttestedEvent, BatchRevokedEvent,
    SasEvent, SchemaRegisteredEvent,
};
use soroban_sdk::{Address, BytesN};

#[test]
fn test_metrics_creation() {
    let metrics = SasMetrics::new();
    assert!(metrics.is_ok());
}

#[test]
fn test_attestation_issued_metric() {
    let metrics = SasMetrics::new().unwrap();
    
    let event = SasEvent::AttestationIssued(AttestationIssuedEvent {
        uid: UID(BytesN::from_array(&[0; 32])),
        schema_uid: UID(BytesN::from_array(&[1; 32])),
        attester: Address::from_string(&"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string()).unwrap(),
        recipient: Address::from_string(&"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string()).unwrap(),
    });
    
    metrics.record_event(&event, "test_contract");
    
    // Verify counter was incremented
    let counter = metrics.attestation_issued_total.get();
    assert_eq!(counter, 1.0);
    
    // Verify gauge was incremented
    let gauge = metrics.active_attestations.get();
    assert_eq!(gauge, 1.0);
}

#[test]
fn test_attestation_revoked_metric() {
    let metrics = SasMetrics::new().unwrap();
    
    let event = SasEvent::AttestationRevoked(AttestationRevokedEvent {
        uid: UID(BytesN::from_array(&[0; 32])),
        timestamp: 12345,
    });
    
    metrics.record_event(&event, "test_contract");
    
    // Verify counter was incremented
    let counter = metrics.attestation_revoked_total.get();
    assert_eq!(counter, 1.0);
    
    // Verify gauge was decremented
    let gauge = metrics.active_attestations.get();
    assert_eq!(gauge, -1.0);
}

#[test]
fn test_batch_attested_metric() {
    let metrics = SasMetrics::new().unwrap();
    
    let event = SasEvent::BatchAttested(BatchAttestedEvent {
        count: 10,
        attester_count: 3,
    });
    
    metrics.record_event(&event, "test_contract");
    
    // Verify batch counter was incremented
    let batch_counter = metrics.batch_attested_total.get();
    assert_eq!(batch_counter, 1.0);
    
    // Verify active attestations gauge was incremented by batch size
    let gauge = metrics.active_attestations.get();
    assert_eq!(gauge, 10.0);
}

#[test]
fn test_schema_registered_metric() {
    let metrics = SasMetrics::new().unwrap();
    
    let event = SasEvent::SchemaRegistered(SchemaRegisteredEvent {
        schema_uid: UID(BytesN::from_array(&[0; 32])),
        owner: Address::from_string(&"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string()).unwrap(),
    });
    
    metrics.record_event(&event, "test_contract");
    
    // Verify counter was incremented
    let counter = metrics.schema_registered_total.get();
    assert_eq!(counter, 1.0);
    
    // Verify gauge was incremented
    let gauge = metrics.active_schemas.get();
    assert_eq!(gauge, 1.0);
}

#[test]
fn test_all_event_types_increment_counters() {
    let metrics = SasMetrics::new().unwrap();
    
    // Test that all event types can be recorded without panicking
    let events = vec![
        SasEvent::AttestationIssued(AttestationIssuedEvent {
            uid: UID(BytesN::from_array(&[0; 32])),
            schema_uid: UID(BytesN::from_array(&[1; 32])),
            attester: Address::from_string(&"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string()).unwrap(),
            recipient: Address::from_string(&"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string()).unwrap(),
        }),
        SasEvent::AttestationRevoked(AttestationRevokedEvent {
            uid: UID(BytesN::from_array(&[0; 32])),
            timestamp: 12345,
        }),
        SasEvent::BatchAttested(BatchAttestedEvent {
            count: 5,
            attester_count: 2,
        }),
        SasEvent::BatchRevoked(BatchRevokedEvent {
            count: 3,
            attester_count: 1,
        }),
        SasEvent::SchemaRegistered(SchemaRegisteredEvent {
            schema_uid: UID(BytesN::from_array(&[0; 32])),
            owner: Address::from_string(&"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_string()).unwrap(),
        }),
    ];
    
    for event in events {
        metrics.record_event(&event, "test_contract");
    }
    
    // Verify counters were incremented
    assert_eq!(metrics.attestation_issued_total.get(), 1.0);
    assert_eq!(metrics.attestation_revoked_total.get(), 1.0);
    assert_eq!(metrics.batch_attested_total.get(), 1.0);
    assert_eq!(metrics.batch_revoked_total.get(), 1.0);
    assert_eq!(metrics.schema_registered_total.get(), 1.0);
}

#[test]
fn test_metrics_registry_contains_all_metrics() {
    let metrics = SasMetrics::new().unwrap();
    
    let metric_families = metrics.registry.gather();
    let metric_names: Vec<&str> = metric_families.iter().map(|m| m.get_name()).collect();
    
    // Verify all expected metrics are present
    assert!(metric_names.contains(&"sas_attestation_issued_total"));
    assert!(metric_names.contains(&"sas_attestation_revoked_total"));
    assert!(metric_names.contains(&"sas_attestation_renewed_total"));
    assert!(metric_names.contains(&"sas_batch_attested_total"));
    assert!(metric_names.contains(&"sas_batch_revoked_total"));
    assert!(metric_names.contains(&"sas_schema_registered_total"));
    assert!(metric_names.contains(&"sas_schema_deprecated_total"));
    assert!(metric_names.contains(&"sas_attester_key_registered_total"));
    assert!(metric_names.contains(&"sas_attester_key_rotated_total"));
    assert!(metric_names.contains(&"sas_attester_key_revoked_total"));
    assert!(metric_names.contains(&"sas_indexer_updated_total"));
    assert!(metric_names.contains(&"sas_indexer_strict_updated_total"));
    assert!(metric_names.contains(&"sas_contract_upgraded_total"));
    assert!(metric_names.contains(&"sas_contract_paused_total"));
    assert!(metric_names.contains(&"sas_contract_unpaused_total"));
    assert!(metric_names.contains(&"sas_admin_transfer_proposed_total"));
    assert!(metric_names.contains(&"sas_admin_transfer_completed_total"));
    assert!(metric_names.contains(&"sas_schema_delegate_added_total"));
    assert!(metric_names.contains(&"sas_schema_delegate_removed_total"));
    assert!(metric_names.contains(&"sas_schema_ownership_transferred_total"));
    assert!(metric_names.contains(&"sas_fee_config_updated_total"));
    assert!(metric_names.contains(&"sas_indexing_progress_total"));
    assert!(metric_names.contains(&"sas_active_attestations"));
    assert!(metric_names.contains(&"sas_active_schemas"));
    assert!(metric_names.contains(&"sas_registered_attester_keys"));
    assert!(metric_names.contains(&"sas_event_processing_duration_seconds"));
}
