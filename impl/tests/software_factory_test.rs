use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use turn::{EffectRecord, FileStore, Runner, Store, ToolRegistry, Value, VmState};

static ENV_LOCK: Mutex<()> = Mutex::new(());
const FACTORY_SOURCE: &str = include_str!("../../examples/software_factory.tn");

struct CrashAfterToolStore {
    inner: FileStore,
    tool_name: String,
    crash_once: bool,
}

impl CrashAfterToolStore {
    fn new(path: &std::path::Path, tool_name: &str) -> Self {
        Self {
            inner: FileStore::new(path),
            tool_name: tool_name.to_string(),
            crash_once: true,
        }
    }
}

impl Store for CrashAfterToolStore {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()> {
        self.inner.save(id, state)
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        self.inner.load(id)
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        self.inner.delete(id)
    }

    fn load_effect(&self, id: &str, effect_id: &str) -> Result<Option<EffectRecord>> {
        self.inner.load_effect(id, effect_id)
    }

    fn save_effect(&mut self, id: &str, record: &EffectRecord) -> Result<()> {
        self.inner.save_effect(id, record)?;
        if self.crash_once && record.tool_name == self.tool_name {
            self.crash_once = false;
            return Err(anyhow!("Simulated factory crash after PR journal commit"));
        }
        Ok(())
    }

    fn list_effects(&self, id: &str) -> Result<Vec<EffectRecord>> {
        self.inner.list_effects(id)
    }
}

fn factory_tools(
    open_pr_calls: Arc<AtomicUsize>,
    observed_effect_ids: Arc<Mutex<Vec<String>>>,
) -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(
        "create_branch",
        Box::new(|_| Ok((Value::Str("factory/issue-42".to_string()), 1))),
    );
    tools.register(
        "apply_patch",
        Box::new(|_| Ok((Value::Str("patch-applied".to_string()), 2))),
    );
    tools.register(
        "run_tests",
        Box::new(|_| Ok((Value::Str("passed".to_string()), 3))),
    );
    tools.register_effectful(
        "open_pull_request",
        Box::new(move |context, _| {
            open_pr_calls.fetch_add(1, Ordering::SeqCst);
            observed_effect_ids
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(context.effect_id.clone());
            Ok((Value::Str("https://example.test/pulls/42".to_string()), 5))
        }),
    );
    tools
}

#[test]
fn software_factory_recovers_without_opening_duplicate_pull_request() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("TURN_LLM_PROVIDER", "mock");
    std::env::set_var("TURN_DECISION_PROVIDER", "mock");

    let directory = tempdir().unwrap();
    let open_pr_calls = Arc::new(AtomicUsize::new(0));
    let observed_effect_ids = Arc::new(Mutex::new(Vec::new()));
    let store = CrashAfterToolStore::new(directory.path(), "open_pull_request");
    let mut runner = Runner::new(
        store,
        factory_tools(open_pr_calls.clone(), observed_effect_ids.clone()),
    );

    let error = runner
        .run("factory", FACTORY_SOURCE, None)
        .expect_err("The injected crash should interrupt the first execution");
    assert_eq!(
        error.to_string(),
        "Simulated factory crash after PR journal commit"
    );
    assert_eq!(open_pr_calls.load(Ordering::SeqCst), 1);

    let store = FileStore::new(directory.path());
    let mut runner = Runner::new(
        store,
        factory_tools(open_pr_calls.clone(), observed_effect_ids.clone()),
    );
    let result = runner.run("factory", FACTORY_SOURCE, None).unwrap();

    let Value::Map(result) = result else {
        panic!("Expected factory result map");
    };
    assert_eq!(
        result.get("status"),
        Some(&Value::Str("pull_request_opened".to_string()))
    );
    assert_eq!(open_pr_calls.load(Ordering::SeqCst), 1);
    let effect_ids = observed_effect_ids
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    assert_eq!(effect_ids.len(), 1);
    assert!(effect_ids[0].ends_with(":1:6"));

    let store = FileStore::new(directory.path());
    let records = store.list_effects("factory").unwrap();
    let tool_names = records
        .iter()
        .map(|record| record.tool_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec![
            "ai_decide",
            "llm_infer",
            "create_branch",
            "apply_patch",
            "run_tests",
            "ai_decide",
            "open_pull_request",
        ]
    );
}
