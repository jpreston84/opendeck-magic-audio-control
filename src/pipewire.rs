use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStream {
    pub node_id: u32,
    pub pid: Option<u32>,
    pub name: String,
    pub app_name: Option<String>,
    pub media_name: Option<String>,
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
            if !parts.is_empty() {
                if let Ok(node_id) = parts[0].parse::<u32>() {
                    let details = self.get_sink_input_details(node_id)?;
                    streams.push(details);
                }
            }
        }

        streams.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.pid.cmp(&b.pid)));

        Ok(streams)
    }

    fn get_sink_input_details(&self, node_id: u32) -> Result<AudioStream> {
        let output = Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut name = format!("Stream {}", node_id);
        let mut app_name = None;
        let mut media_name = None;
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

            if line.starts_with("media.title = ") {
                let val = line.trim_start_matches("media.title = ");
                let val = val.trim_matches('"').trim();
                if !val.is_empty() {
                    media_name = Some(val.to_string());
                    if app_name.is_none() {
                        name = val.to_string();
                    }
                }
            }

            if line.starts_with("media.name = ") {
                let val = line.trim_start_matches("media.name = ");
                let val = val.trim_matches('"').trim();
                if !val.is_empty() {
                    if media_name.is_none() {
                        media_name = Some(val.to_string());
                    }
                    if app_name.is_none() {
                        name = val.to_string();
                    }
                }
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
            media_name,
        })
    }

    pub fn get_stream_volume(node_id: u32) -> Result<u32> {
        let output = Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current_input: Option<u32> = None;
        let mut volume: u32 = 100;

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

            if line.starts_with("Volume:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in &parts {
                    if part.contains('%') {
                        let pct = part.trim_end_matches('%');
                        if let Ok(v) = pct.parse::<u32>() {
                            volume = v;
                            break;
                        }
                    }
                }
                break;
            }
        }

        Ok(volume)
    }

    pub fn set_stream_volume(node_id: u32, volume: u32) -> Result<()> {
        let volume = volume.clamp(0, 150);

        Command::new("pactl")
            .args([
                "set-sink-input-volume",
                &node_id.to_string(),
                &format!("{}%", volume),
            ])
            .output()?;

        Ok(())
    }

    pub fn adjust_stream_volume(node_id: u32, delta_percent: i32) -> Result<u32> {
        let current = Self::get_stream_volume(node_id)?;
        let new_volume = (current as i32 + delta_percent).clamp(0, 150) as u32;
        Self::set_stream_volume(node_id, new_volume)?;
        Ok(new_volume)
    }

    pub fn get_master_volume() -> Result<u32> {
        let output = Command::new("pactl")
            .args(["get-sink-volume", "@DEFAULT_SINK@"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for part in stdout.split_whitespace() {
            if part.contains('%') {
                let pct = part.trim_end_matches('%');
                if let Ok(v) = pct.parse::<u32>() {
                    return Ok(v);
                }
            }
        }
        Ok(100)
    }

    pub fn set_master_volume(volume: u32) -> Result<()> {
        let volume = volume.clamp(0, 150);
        Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", volume)])
            .output()?;
        Ok(())
    }

    pub fn adjust_master_volume(delta_percent: i32) -> Result<u32> {
        let current = Self::get_master_volume()?;
        let new_volume = (current as i32 + delta_percent).clamp(0, 150) as u32;
        Self::set_master_volume(new_volume)?;
        Ok(new_volume)
    }

    pub fn get_mic_volume() -> Result<u32> {
        let output = Command::new("pactl")
            .args(["get-source-volume", "@DEFAULT_SOURCE@"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for part in stdout.split_whitespace() {
            if part.contains('%') {
                let pct = part.trim_end_matches('%');
                if let Ok(v) = pct.parse::<u32>() {
                    return Ok(v);
                }
            }
        }
        Ok(100)
    }

    pub fn set_mic_volume(volume: u32) -> Result<()> {
        let volume = volume.clamp(0, 150);
        Command::new("pactl")
            .args([
                "set-source-volume",
                "@DEFAULT_SOURCE@",
                &format!("{}%", volume),
            ])
            .output()?;
        Ok(())
    }

    pub fn adjust_mic_volume(delta_percent: i32) -> Result<u32> {
        let current = Self::get_mic_volume()?;
        let new_volume = (current as i32 + delta_percent).clamp(0, 150) as u32;
        Self::set_mic_volume(new_volume)?;
        Ok(new_volume)
    }

    pub fn get_stream_mute(node_id: u32) -> Result<bool> {
        let output = Command::new("pactl")
            .args(["list", "sink-inputs"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current_input: Option<u32> = None;
        let mut muted = false;

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

            if line.contains("Mute:") {
                muted = line.contains("yes");
                break;
            }
        }

        Ok(muted)
    }

    pub fn set_stream_mute(node_id: u32, mute: bool) -> Result<()> {
        let mute_str = if mute { "1" } else { "0" };
        Command::new("pactl")
            .args(["set-sink-input-mute", &node_id.to_string(), mute_str])
            .output()?;
        Ok(())
    }

    pub fn toggle_stream_mute(node_id: u32) -> Result<bool> {
        let current = Self::get_stream_mute(node_id)?;
        let new_mute = !current;
        Self::set_stream_mute(node_id, new_mute)?;
        Ok(new_mute)
    }

    pub fn get_master_mute() -> Result<bool> {
        let output = Command::new("pactl")
            .args(["get-sink-mute", "@DEFAULT_SINK@"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("yes"))
    }

    pub fn set_master_mute(mute: bool) -> Result<()> {
        let mute_str = if mute { "1" } else { "0" };
        Command::new("pactl")
            .args(["set-sink-mute", "@DEFAULT_SINK@", mute_str])
            .output()?;
        Ok(())
    }

    pub fn toggle_master_mute() -> Result<bool> {
        let current = Self::get_master_mute()?;
        let new_mute = !current;
        Self::set_master_mute(new_mute)?;
        Ok(new_mute)
    }

    pub fn get_mic_mute() -> Result<bool> {
        let output = Command::new("pactl")
            .args(["get-source-mute", "@DEFAULT_SOURCE@"])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("yes"))
    }

    pub fn set_mic_mute(mute: bool) -> Result<()> {
        let mute_str = if mute { "1" } else { "0" };
        Command::new("pactl")
            .args(["set-source-mute", "@DEFAULT_SOURCE@", mute_str])
            .output()?;
        Ok(())
    }

    pub fn toggle_mic_mute() -> Result<bool> {
        let current = Self::get_mic_mute()?;
        let new_mute = !current;
        Self::set_mic_mute(new_mute)?;
        Ok(new_mute)
    }
}

impl Default for PipeWireManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize PipeWire manager")
    }
}
