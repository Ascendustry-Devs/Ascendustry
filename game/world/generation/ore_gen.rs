use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct OreVeinConfig {
    pub block_id: String,
    pub depth_min: i32,
    pub depth_max: i32,
    pub noise_scale: f64,
    pub threshold: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OreGenConfig {
    pub ores: Vec<OreVeinConfig>,
}

impl OreGenConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content =
            fs::read_to_string(path.as_ref()).map_err(|e| format!("Failed to read ore config {:?}: {}", path.as_ref(), e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse ore config {:?}: {}", path.as_ref(), e))
    }
}
