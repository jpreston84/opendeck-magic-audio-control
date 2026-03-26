use crate::pipewire::PipeWireManager;
use openaction::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct CycleAudioStreamSettings {
    pub current_pid: Option<u32>,
    pub current_name: Option<String>,
}

pub struct CycleAudioStreamAction;

#[async_trait]
impl Action for CycleAudioStreamAction {
    const UUID: &'static str = "net.jpreston.opendeck.audio.cycle-stream";
    type Settings = CycleAudioStreamSettings;

    async fn key_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let pw = match PipeWireManager::new() {
            Ok(pw) => pw,
            Err(e) => {
                log::error!("Failed to initialize PipeWire: {}", e);
                instance.set_title(Some("PipeWire Error".to_string()), None).await?;
                return Ok(());
            }
        };

        let streams = match pw.get_active_streams() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to get streams: {}", e);
                instance.set_title(Some("Stream Error".to_string()), None).await?;
                return Ok(());
            }
        };

        if streams.is_empty() {
            instance.set_title(Some("No Streams".to_string()), None).await?;
            return Ok(());
        }

        let next_stream = if let Some(current_pid) = settings.current_pid {
            let current_idx = streams
                .iter()
                .position(|s| s.pid == Some(current_pid))
                .unwrap_or(0);

            let next_idx = (current_idx + 1) % streams.len();
            &streams[next_idx]
        } else {
            &streams[0]
        };

        let new_settings = CycleAudioStreamSettings {
            current_pid: next_stream.pid,
            current_name: Some(next_stream.name.clone()),
        };

        instance.set_settings(&new_settings).await?;

        let title = next_stream
            .app_name
            .as_ref()
            .unwrap_or(&next_stream.name);
        instance.set_title(Some(title.clone()), None).await?;

        log::info!(
            "Cycled to stream: {} (PID: {:?})",
            next_stream.name,
            next_stream.pid
        );

        Ok(())
    }

    async fn will_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        if let Some(name) = &settings.current_name {
            instance.set_title(Some(name.clone()), None).await?;
        } else {
            instance.set_title(Some("Cycle Audio".to_string()), None).await?;
        }
        Ok(())
    }

    async fn property_inspector_did_appear(
        &self,
        instance: &Instance,
        _settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let pw = match PipeWireManager::new() {
            Ok(pw) => pw,
            Err(e) => {
                log::error!("Failed to initialize PipeWire: {}", e);
                return Ok(());
            }
        };

        let streams = match pw.get_active_streams() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to get streams: {}", e);
                return Ok(());
            }
        };

        let stream_info: Vec<serde_json::Value> = streams
            .iter()
            .map(|s| {
                serde_json::json!({
                    "node_id": s.node_id,
                    "pid": s.pid,
                    "name": s.name,
                    "app_name": s.app_name,
                })
            })
            .collect();

        instance
            .send_to_property_inspector(serde_json::json!({
                "event": "updateStreams",
                "streams": stream_info,
            }))
            .await?;

        Ok(())
    }
}
