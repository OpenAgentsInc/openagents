use std::collections::HashMap;
use std::path::PathBuf;

// Define the Forge knowledge-pack object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePack {
    pub id: String,
    pub kind: PackKind,
    pub provenance: Provenance,
    pub source_reference: SourceReference,
    pub content: String,
}

// Define the pack kinds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackKind {
    RepoDocs,
    Runbooks,
    SessionSummaries,
    PatchSummaries,
    BenchmarkReferences,
    JudgePackReferences,
}

// Define the provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub created_at: String,
    pub updated_at: String,
}

// Define the source reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReference {
    pub url: String,
    pub commit_hash: String,
}

// Define the catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackCatalog {
    pub packs: HashMap<String, KnowledgePack>,
}

impl PackCatalog {
    pub fn new() -> Self {
        PackCatalog {
            packs: HashMap::new(),
        }
    }

    pub fn add_pack(&mut self, pack: KnowledgePack) {
        self.packs.insert(pack.id.clone(), pack);
    }

    pub fn get_pack(&self, id: &str) -> Option<&KnowledgePack> {
        self.packs.get(id)
    }

    pub fn remove_pack(&mut self, id: &str) -> Option<KnowledgePack> {
        self.packs.remove(id)
    }
}

// Define the app state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub pack_catalog: PackCatalog,
    pub current_project: Option<String>,
    pub current_workspace: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            pack_catalog: PackCatalog::new(),
            current_project: None,
            current_workspace: None,
        }
    }

    pub fn load_from_file(&mut self, path: &PathBuf) -> Result<(), std::io::Error> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut state: AppState = serde_json::from_reader(reader)?;
        self.pack_catalog = state.pack_catalog;
        self.current_project = state.current_project;
        self.current_workspace = state.current_workspace;
        Ok(())
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let file = std::fs::File::create(path)?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }
}