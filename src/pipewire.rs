use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStream {
    pub node_id: u32,
    pub pid: Option<u32>,
    pub name: String,
    pub app_name: Option<String>,
}

pub struct PipeWireManager;

impl PipeWireManager {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn get_active_streams(&self) -> Result<Vec<AudioStream>> {
        let output = Command::new("pactl")
            .args(["list", "sink-inputs", "short"])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("pactl command failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut streams = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 1 {
                if let Ok(node_id) = parts[0].parse::<u32>() {
                    let details = self.get_sink_input_details(node_id)?;
                    streams.push(details);
                }
            }
        }

        streams.sort_by(|a, b| {
            a.name.cmp(&b.name).then_with(|| a.pid.cmp(&b.pid))
        });

        Ok(streams)
    }

    fn get_sink_input_details(&self, node_id: u32) -> Result<AudioStream> {
        let output = Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut name = format!("Stream {}", node_id);
        let mut app_name = None;
        let mut pid = None;

        let mut current_input: Option<u32> = None;

        for line in stdout.lines() {
            let line = line.trim();

            if line.starts_with("Sink Input #") {
                let id_str = line.trim_start_matches("Sink Input #");
                current_input = id_str.parse::<u32>().ok();
                continue;
            }

            if current_input != Some(node_id) {
                continue;
            }

            if line.starts_with("application.name = ") {
                let val = line.trim_start_matches("application.name = ");
                let val = val.trim_matches('"').trim();
                app_name = Some(val.to_string());
                name = val.to_string();
            }

            if line.starts_with("application.process.id = ") {
                let val = line.trim_start_matches("application.process.id = ");
                let val = val.trim_matches('"').trim();
                pid = val.parse::<u32>().ok();
            }

            if line.starts_with("media.name = ") && app_name.is_none() {
                let val = line.trim_start_matches("media.name = ");
                let val = val.trim_matches('"').trim();
                name = val.to_string();
            }

            if line.starts_with("Sink Input #") && current_input == Some(node_id) {
                break;
            }
        }

        Ok(AudioStream {
            node_id,
            pid,
            name,
            app_name,
        })
    }
}

impl Default for PipeWireManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize PipeWire manager")
    }
}
