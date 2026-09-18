use crate::value::Value;
use crate::vm::VmState;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EffectOutcome {
    Success { value: Value, cost: u64 },
    Failure { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectRecord {
    pub effect_id: String,
    pub tool_name: String,
    pub arg: Value,
    pub outcome: EffectOutcome,
    pub completed_at_ms: u128,
}

pub trait Store {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()>;
    fn load(&self, id: &str) -> Result<Option<VmState>>;
    fn delete(&mut self, _id: &str) -> Result<()> {
        Ok(())
    }
    fn load_effect(&self, _id: &str, _effect_id: &str) -> Result<Option<EffectRecord>> {
        Ok(None)
    }
    fn save_effect(&mut self, _id: &str, _record: &EffectRecord) -> Result<()> {
        Ok(())
    }
    fn list_effects(&self, _id: &str) -> Result<Vec<EffectRecord>> {
        Ok(Vec::new())
    }
}

pub struct FileStore {
    base_path: std::path::PathBuf,
}

impl FileStore {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        let path = base_path.into();
        if !path.exists() {
            std::fs::create_dir_all(&path).unwrap();
        }
        Self { base_path: path }
    }

    fn safe_component(value: &str) -> String {
        let mut encoded = String::new();
        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
                encoded.push(byte as char);
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        encoded
    }

    fn state_path(&self, id: &str) -> std::path::PathBuf {
        self.base_path
            .join(format!("{}.json", Self::safe_component(id)))
    }

    fn effects_path(&self, id: &str) -> std::path::PathBuf {
        self.base_path
            .join(format!("{}.effects", Self::safe_component(id)))
    }

    fn effect_path(&self, id: &str, effect_id: &str) -> std::path::PathBuf {
        self.effects_path(id)
            .join(format!("{}.json", Self::safe_component(effect_id)))
    }

    fn write_atomic<T: Serialize>(&self, path: &std::path::Path, value: &T) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Store path has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer(&mut temporary, value)?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

impl Store for FileStore {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()> {
        self.write_atomic(&self.state_path(id), state)
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        let path = self.state_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(path)?;
        let state: VmState = serde_json::from_reader(file)?;
        Ok(Some(state))
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        let path = self.state_path(id);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn load_effect(&self, id: &str, effect_id: &str) -> Result<Option<EffectRecord>> {
        let path = self.effect_path(id, effect_id);
        if !path.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(path)?;
        Ok(Some(serde_json::from_reader(file)?))
    }

    fn save_effect(&mut self, id: &str, record: &EffectRecord) -> Result<()> {
        let path = self.effect_path(id, &record.effect_id);
        if path.exists() {
            let existing = self
                .load_effect(id, &record.effect_id)?
                .ok_or_else(|| anyhow::anyhow!("Effect record disappeared"))?;
            if existing != *record {
                anyhow::bail!("Effect ID collision: {}", record.effect_id);
            }
            return Ok(());
        }
        self.write_atomic(&path, record)
    }

    fn list_effects(&self, id: &str) -> Result<Vec<EffectRecord>> {
        let directory = self.effects_path(id);
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut paths =
            std::fs::read_dir(directory)?.collect::<std::result::Result<Vec<_>, _>>()?;
        paths.sort_by_key(|entry| entry.file_name());
        let mut records = paths
            .into_iter()
            .map(|entry| {
                let file = std::fs::File::open(entry.path())?;
                Ok(serde_json::from_reader(file)?)
            })
            .collect::<Result<Vec<EffectRecord>>>()?;
        records.sort_by(|left, right| {
            left.completed_at_ms
                .cmp(&right.completed_at_ms)
                .then_with(|| left.effect_id.cmp(&right.effect_id))
        });
        Ok(records)
    }
}
