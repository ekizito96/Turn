use crate::vm::VmState;
use anyhow::Result;

pub trait Store {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()>;
    fn load(&self, id: &str) -> Result<Option<VmState>>;
    
    // NEW Phase 5 Pillar 3: Time-Travel Replay
    fn load_version(&self, _id: &str, _version: usize) -> Result<Option<VmState>> {
        Ok(None)
    }
    fn list_versions(&self, _id: &str) -> Result<Vec<usize>> {
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
}

impl Store for FileStore {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()> {
        let dir_path = self.base_path.join(id);
        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path)?;
        }
        
        let mut latest_version = 0;
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".json") && name != "latest.json" {
                    if let Ok(v) = name.trim_end_matches(".json").parse::<usize>() {
                        latest_version = latest_version.max(v + 1);
                    }
                }
            }
        }

        let version_path = dir_path.join(format!("{}.json", latest_version));
        let file = std::fs::File::create(&version_path)?;
        serde_json::to_writer(file, state)?;

        // Maintain current execution snapshot
        let latest_path = dir_path.join("latest.json");
        let latest_file = std::fs::File::create(&latest_path)?;
        serde_json::to_writer(latest_file, state)?;

        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        let path = self.base_path.join(id).join("latest.json");
        if !path.exists() {
            // Legacy backward compatibility for pre-Time-Travel WAL snapshots
            let legacy_path = self.base_path.join(format!("{}.json", id));
            if legacy_path.exists() {
                let file = std::fs::File::open(legacy_path)?;
                let state: VmState = serde_json::from_reader(file)?;
                return Ok(Some(state));
            }
            return Ok(None);
        }
        let file = std::fs::File::open(path)?;
        let state: VmState = serde_json::from_reader(file)?;
        Ok(Some(state))
    }

    fn load_version(&self, id: &str, version: usize) -> Result<Option<VmState>> {
        let path = self.base_path.join(id).join(format!("{}.json", version));
        if !path.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(path)?;
        let state: VmState = serde_json::from_reader(file)?;
        Ok(Some(state))
    }

    fn list_versions(&self, id: &str) -> Result<Vec<usize>> {
        let dir_path = self.base_path.join(id);
        let mut versions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".json") && name != "latest.json" {
                    if let Ok(v) = name.trim_end_matches(".json").parse::<usize>() {
                        versions.push(v);
                    }
                }
            }
        }
        versions.sort_unstable();
        Ok(versions)
    }
}
