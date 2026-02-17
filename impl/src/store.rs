use crate::vm::VmState;
use anyhow::Result;

pub trait Store {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()>;
    fn load(&self, id: &str) -> Result<Option<VmState>>;
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
        let path = self.base_path.join(format!("{}.json", id));
        let file = std::fs::File::create(path)?;
        serde_json::to_writer(file, state)?;
        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        let path = self.base_path.join(format!("{}.json", id));
        if !path.exists() {
            return Ok(None);
        }
        let file = std::fs::File::open(path)?;
        let state: VmState = serde_json::from_reader(file)?;
        Ok(Some(state))
    }
}
