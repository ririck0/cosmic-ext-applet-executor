use std::fs;

pub const APP_ID: &str = "io.github.cosmic_utils.cosmic-ext-applet-executor";

#[derive(Debug, Clone)]
pub struct BlockConfig {
    pub command: String,
    pub interval: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    pub blocks: Vec<BlockConfig>,
    pub separator: String,
    pub font_size: Option<f32>,
}

impl ExecutorConfig {
    pub fn config() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("cosmic/io.github.cosmic_utils.cosmic-ext-applet-executor/v1/config.json");

        let Some(v) = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        else {
            return ExecutorConfig::default();
        };

        let separator = v.get("separator")
            .and_then(|s| s.as_str())
            .unwrap_or("|")
            .to_string();

        let font_size = v.get("font_size").and_then(|f| f.as_f64()).map(|f| f as f32);

        let blocks = v.get("blocks")
            .and_then(|b| b.as_array())
            .map(|arr| {
                arr.iter().filter_map(|entry| {
                    let command = entry.get("command")?.as_str()?.to_string();
                    if command.is_empty() { return None; }
                    let interval = entry.get("interval").and_then(|i| i.as_u64()).unwrap_or(5);
                    Some(BlockConfig { command, interval })
                }).collect()
            })
            .unwrap_or_default();

        ExecutorConfig { blocks, separator, font_size }
    }
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        ExecutorConfig {
            blocks: Vec::new(),
            separator: "|".to_string(),
            font_size: None,
        }
    }
}
