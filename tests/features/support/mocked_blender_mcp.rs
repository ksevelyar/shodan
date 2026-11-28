use std::path::{Path, PathBuf};

use serde_json::Value;

pub struct MockedBlenderMcp {
    request_path: PathBuf,
}

impl MockedBlenderMcp {
    pub fn new(directory: &Path) -> Self {
        Self {
            request_path: directory.join("blender-mcp-requests.jsonl"),
        }
    }

    pub fn command(&self) -> Vec<String> {
        let fixtures =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features/fixtures/mcp/blender");
        vec![
            "python3".to_owned(),
            fixtures.join("replay.py").display().to_string(),
            fixtures.join("get-scene-info.jsonl").display().to_string(),
            self.request_path.display().to_string(),
        ]
    }

    pub fn received_requests(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.request_path)
            .expect("MCP requests are recorded")
            .lines()
            .map(|line| serde_json::from_str(line).expect("MCP request is valid JSON"))
            .collect()
    }
}
