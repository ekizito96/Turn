use tempfile::tempdir;
use turn::{EffectOutcome, EffectRecord, FileStore, Store, Value};

fn record(effect_id: &str, completed_at_ms: u128) -> EffectRecord {
    EffectRecord {
        effect_id: effect_id.to_string(),
        tool_name: "create_pull_request".to_string(),
        arg: Value::Str("feature/factory".to_string()),
        outcome: EffectOutcome::Success {
            value: Value::Str("pr-42".to_string()),
            cost: 7,
        },
        completed_at_ms,
    }
}

#[test]
fn file_store_round_trips_and_orders_effect_records() {
    let directory = tempdir().unwrap();
    let mut store = FileStore::new(directory.path());
    store
        .save_effect("factory/run", &record("run:1", 20))
        .unwrap();
    store
        .save_effect("factory/run", &record("run:0", 10))
        .unwrap();

    let records = store.list_effects("factory/run").unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].effect_id, "run:0");
    assert_eq!(records[1].effect_id, "run:1");
    assert_eq!(
        store.load_effect("factory/run", "run:1").unwrap(),
        Some(record("run:1", 20))
    );
    assert!(!directory.path().join("factory").exists());
}

#[test]
fn file_store_accepts_identical_record_and_rejects_collision() {
    let directory = tempdir().unwrap();
    let mut store = FileStore::new(directory.path());
    let original = record("run:0", 10);
    store.save_effect("factory", &original).unwrap();
    store.save_effect("factory", &original).unwrap();

    let mut collision = original;
    collision.tool_name = "merge_pull_request".to_string();
    let error = store.save_effect("factory", &collision).unwrap_err();
    assert!(error.to_string().contains("Effect ID collision: run:0"));
}
