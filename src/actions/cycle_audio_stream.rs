use crate::audio::{AudioService, AudioTarget};
use crate::constants::{LONG_PRESS_DURATION, MASTER_NODE_ID, MIC_NODE_ID, OSCILLATE_INTERVAL};
use crate::display::{
    blank_button_image, clear_knob_instances, set_button_instance_icon, unlocked_button_image,
    update_knob_instances,
};
use crate::pipewire::AudioStream;
use crate::state::{StateRepo, StreamSelection};
use openaction::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tokio::time::sleep;

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct CycleAudioStreamSettings {
    pub link_id: Option<String>,
    pub current_node_id: Option<u32>,
    pub current_name: Option<String>,
    pub current_app_name: Option<String>,
    pub is_blank: bool,
}

pub struct CycleAudioStreamAction;

fn generate_link_id(instance: &Instance) -> String {
    if let Some(coords) = &instance.coordinates {
        format!("btn-{}-{}", coords.row, coords.column)
    } else {
        format!("btn-{}", instance.instance_id)
    }
}

fn resolve_link_id(instance: &Instance, settings: &CycleAudioStreamSettings) -> String {
    settings
        .link_id
        .clone()
        .or_else(|| StateRepo::find_button_link_id(&instance.instance_id))
        .unwrap_or_else(|| generate_link_id(instance))
}

fn selection_from_settings(settings: &CycleAudioStreamSettings) -> Option<StreamSelection> {
    let node_id = settings.current_node_id?;
    let name = settings
        .current_name
        .clone()
        .unwrap_or_else(|| match node_id {
            MASTER_NODE_ID => "Master".to_string(),
            MIC_NODE_ID => "Mic".to_string(),
            _ => format!("Stream {}", node_id),
        });

    Some(StreamSelection {
        node_id,
        name,
        app_name: settings.current_app_name.clone(),
        media_name: None,
    })
}

fn effective_selection(
    link_id: &str,
    settings: &CycleAudioStreamSettings,
) -> Option<StreamSelection> {
    if settings.is_blank {
        return None;
    }

    StateRepo::load_selection(link_id).or_else(|| selection_from_settings(settings))
}

fn effective_node_id(link_id: &str, settings: &CycleAudioStreamSettings) -> Option<u32> {
    effective_selection(link_id, settings).map(|selection| selection.node_id)
}

async fn cycle_stream(
    instance: &Instance,
    settings: &CycleAudioStreamSettings,
    link_id: &str,
) -> OpenActionResult<()> {
    let effective_node_id = effective_node_id(link_id, settings);

    let streams: Vec<AudioStream> = match AudioService::list_streams() {
        Ok(s) => s,
        Err(e) => {
            log::error!("[ACTION] Failed to get streams: {}", e);
            return Ok(());
        }
    };

    log::info!(
        "[ACTION] Current node: {:?}, streams count: {}",
        effective_node_id,
        streams.len()
    );

    let (next_node_id, next_name, next_app_name, next_media_name, go_blank) =
        match effective_node_id {
            None => (
                MASTER_NODE_ID,
                "Master".to_string(),
                Some("Master".to_string()),
                None,
                false,
            ),
            Some(id) if id == MASTER_NODE_ID => (
                MIC_NODE_ID,
                "Mic".to_string(),
                Some("Mic".to_string()),
                None,
                false,
            ),
            Some(id) if id == MIC_NODE_ID => {
                if streams.is_empty() {
                    (
                        MASTER_NODE_ID,
                        "Master".to_string(),
                        Some("Master".to_string()),
                        None,
                        true,
                    )
                } else {
                    (
                        streams[0].node_id,
                        streams[0].name.clone(),
                        streams[0].app_name.clone(),
                        streams[0].media_name.clone(),
                        false,
                    )
                }
            }
            Some(id) => match streams.iter().position(|s| s.node_id == id) {
                Some(current_idx) if current_idx < streams.len() - 1 => {
                    let next = &streams[current_idx + 1];
                    (
                        next.node_id,
                        next.name.clone(),
                        next.app_name.clone(),
                        next.media_name.clone(),
                        false,
                    )
                }
                _ => (
                    MASTER_NODE_ID,
                    "Master".to_string(),
                    Some("Master".to_string()),
                    None,
                    true,
                ),
            },
        };

    log::info!(
        "[ACTION] Next: node_id={}, name={}, go_blank={}",
        next_node_id,
        next_name,
        go_blank
    );

    if go_blank {
        let new_settings = CycleAudioStreamSettings {
            link_id: Some(link_id.to_string()),
            current_node_id: None,
            current_name: None,
            current_app_name: None,
            is_blank: true,
        };
        instance.set_settings(&new_settings).await?;

        let blank_image = blank_button_image();
        StateRepo::update_button_icon(link_id, blank_image.clone()).await;
        instance.set_image(Some(blank_image), None).await?;
        clear_knob_instances(StateRepo::get_knobs_for_button(link_id)).await;
        StateRepo::clear_selection(link_id);
        StateRepo::update_button_name(link_id, None);
        return Ok(());
    }

    apply_selection(
        instance,
        link_id,
        next_node_id,
        next_name,
        next_app_name,
        next_media_name,
    )
    .await
}

async fn apply_selection(
    instance: &Instance,
    link_id: &str,
    node_id: u32,
    name: String,
    app_name: Option<String>,
    media_name: Option<String>,
) -> OpenActionResult<()> {
    let new_settings = CycleAudioStreamSettings {
        link_id: Some(link_id.to_string()),
        current_node_id: Some(node_id),
        current_name: Some(name.clone()),
        current_app_name: app_name.clone(),
        is_blank: false,
    };

    instance.set_settings(&new_settings).await?;

    let target = AudioTarget::from_node_id(node_id);
    let volume = AudioService::get_volume(target).unwrap_or(100);
    let is_muted = AudioService::get_mute(target).unwrap_or(false);

    log::info!(
        "[ACTION] Volume for node {}: {}, muted: {}",
        node_id,
        volume,
        is_muted
    );

    let display_info = target.display_info(&name, app_name.as_deref(), media_name.as_deref());
    let knob_title = display_info.knob_title();
    update_knob_instances(
        StateRepo::get_knobs_for_button(link_id),
        &knob_title,
        volume,
        is_muted,
    )
    .await;

    StateRepo::save_selection(
        link_id,
        &StreamSelection {
            node_id,
            name: name.clone(),
            app_name: app_name.clone(),
            media_name: media_name.clone(),
        },
    );
    StateRepo::update_button_name(link_id, Some(&name));

    if let Some(icon) =
        set_button_instance_icon(instance, &display_info.icon_query, is_muted).await?
    {
        StateRepo::update_button_icon(link_id, icon).await;
    }

    Ok(())
}

pub async fn assign_stream_to_button(
    instance: &Instance,
    link_id: &str,
    stream: &AudioStream,
) -> OpenActionResult<CycleAudioStreamSettings> {
    apply_selection(
        instance,
        link_id,
        stream.node_id,
        stream.name.clone(),
        stream.app_name.clone(),
        stream.media_name.clone(),
    )
    .await?;

    Ok(CycleAudioStreamSettings {
        link_id: Some(link_id.to_string()),
        current_node_id: Some(stream.node_id),
        current_name: Some(stream.name.clone()),
        current_app_name: stream.app_name.clone(),
        is_blank: false,
    })
}

async fn switch_to_recent_stream(
    instance: &Instance,
    link_id: &str,
) -> OpenActionResult<CycleAudioStreamSettings> {
    let Some(stream) = AudioService::most_recent_stream().unwrap_or(None) else {
        return Ok(CycleAudioStreamSettings {
            link_id: Some(link_id.to_string()),
            current_node_id: None,
            current_name: None,
            current_app_name: None,
            is_blank: true,
        });
    };

    assign_stream_to_button(instance, link_id, &stream).await
}

async fn toggle_mute(
    instance: &Instance,
    _settings: &CycleAudioStreamSettings,
    link_id: &str,
) -> OpenActionResult<()> {
    let selection = match StateRepo::load_selection(link_id) {
        Some(s) => s,
        None => return Ok(()),
    };

    let target = AudioTarget::from_node_id(selection.node_id);
    let muted = AudioService::toggle_mute(target).unwrap_or(false);
    let volume = AudioService::get_volume(target).unwrap_or(100);

    let display_info = target.display_info(
        &selection.name,
        selection.app_name.as_deref(),
        selection.media_name.as_deref(),
    );
    let knob_title = display_info.knob_title();
    update_knob_instances(
        StateRepo::get_knobs_for_button(link_id),
        &knob_title,
        volume,
        muted,
    )
    .await;

    if let Some(icon) = set_button_instance_icon(instance, &display_info.icon_query, muted).await? {
        StateRepo::update_button_icon(link_id, icon).await;
    }

    log::info!("[ACTION] Toggle mute: muted={}, volume={}", muted, volume);

    Ok(())
}

fn start_oscillation(instance_id: String, link_id: String, settings: CycleAudioStreamSettings) {
    tokio::spawn(async move {
        let mut is_light = false;

        let initial_icon = if settings.is_blank {
            Some(blank_button_image())
        } else if let Some(node_id) = settings.current_node_id {
            let target = AudioTarget::from_node_id(node_id);
            let display_info = target.display_info(
                settings.current_name.as_deref().unwrap_or(""),
                settings.current_app_name.as_deref(),
                None,
            );
            crate::display::button_icon_image(&display_info.icon_query, false).await
        } else {
            Some(blank_button_image())
        };

        if let Some(ref icon) = initial_icon {
            StateRepo::update_button_icon(&link_id, icon.clone()).await;
        }

        loop {
            let state = StateRepo::get_button_lock_state(&link_id).await;

            if !state.is_unlocked {
                if let Some(instance) = get_instance(instance_id.clone()).await {
                    if let Some(icon) = StateRepo::get_button_icon(&link_id).await {
                        instance.set_image(Some(icon), None).await.ok();
                    } else {
                        instance
                            .set_image(Some(blank_button_image()), None)
                            .await
                            .ok();
                    }
                }
                break;
            }

            if let Some(expires) = state.unlock_expires {
                if Instant::now() >= expires {
                    StateRepo::lock_button(&link_id).await;

                    if let Some(instance) = get_instance(instance_id.clone()).await {
                        if let Some(icon) = StateRepo::get_button_icon(&link_id).await {
                            instance.set_image(Some(icon), None).await.ok();
                        } else {
                            instance
                                .set_image(Some(blank_button_image()), None)
                                .await
                                .ok();
                        }
                    }
                    break;
                }
            }

            is_light = !is_light;

            if let Some(icon) = StateRepo::get_button_icon(&link_id).await {
                let bg = unlocked_button_image(&icon, is_light);

                if let Some(instance) = get_instance(instance_id.clone()).await {
                    instance.set_image(Some(bg), None).await.ok();
                }
            }

            sleep(OSCILLATE_INTERVAL).await;
        }
    });
}

#[async_trait]
impl Action for CycleAudioStreamAction {
    const UUID: &'static str = "net.jpreston.opendeck.audio.cycle-stream";
    type Settings = CycleAudioStreamSettings;

    async fn key_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let link_id = resolve_link_id(instance, settings);
        StateRepo::record_press_start(&link_id).await;
        Ok(())
    }

    async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
        let link_id = resolve_link_id(instance, settings);
        let is_effectively_blank =
            settings.is_blank || effective_node_id(&link_id, settings).is_none();

        let press_start = StateRepo::clear_press_start(&link_id).await;
        let is_unlocked = StateRepo::is_button_unlocked(&link_id).await;

        if is_effectively_blank && !is_unlocked {
            log::info!(
                "[ACTION] Blank button pressed, assigning most recent stream and unlocking {}",
                link_id
            );
            let unlocked_settings = switch_to_recent_stream(instance, &link_id).await?;
            StateRepo::unlock_button(&link_id).await;
            start_oscillation(
                instance.instance_id.clone(),
                link_id.clone(),
                unlocked_settings,
            );
            return Ok(());
        }

        if let Some(start) = press_start {
            let press_duration = start.elapsed();

            if press_duration >= LONG_PRESS_DURATION && !is_unlocked {
                log::info!("[ACTION] Long press detected, unlocking button {}", link_id);
                StateRepo::unlock_button(&link_id).await;
                start_oscillation(
                    instance.instance_id.clone(),
                    link_id.clone(),
                    settings.clone(),
                );
                return Ok(());
            }
        }

        if is_unlocked {
            log::info!("[ACTION] Button unlocked, cycling stream");
            StateRepo::refresh_unlock_timer(&link_id).await;
            cycle_stream(instance, settings, &link_id).await?;
        } else if !is_effectively_blank {
            toggle_mute(instance, settings, &link_id).await?;
        }

        Ok(())
    }

    async fn will_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let link_id = resolve_link_id(instance, settings);

        if settings.link_id.is_none() {
            let new_settings = CycleAudioStreamSettings {
                link_id: Some(link_id.clone()),
                ..settings.clone()
            };
            instance.set_settings(&new_settings).await?;
        }

        let current_name = if settings.is_blank {
            None
        } else {
            settings.current_name.as_deref()
        };
        StateRepo::register_button(&link_id, &instance.instance_id, current_name);

        if settings.is_blank {
            StateRepo::clear_selection(&link_id);
            StateRepo::update_button_name(&link_id, None);
            let blank_image = blank_button_image();
            StateRepo::update_button_icon(&link_id, blank_image.clone()).await;
            instance.set_image(Some(blank_image), None).await?;
            return Ok(());
        }

        if let Some(node_id) = settings.current_node_id {
            if StateRepo::load_selection(&link_id).is_none() {
                if let Some(selection) = selection_from_settings(settings) {
                    StateRepo::save_selection(&link_id, &selection);
                }
            }

            let target = AudioTarget::from_node_id(node_id);
            let is_muted = AudioService::get_mute(target).unwrap_or(false);
            let display_info = target.display_info(
                settings.current_name.as_deref().unwrap_or(""),
                settings.current_app_name.as_deref(),
                None,
            );

            if let Some(icon) =
                set_button_instance_icon(instance, &display_info.icon_query, is_muted).await?
            {
                StateRepo::update_button_icon(&link_id, icon).await;
            }
        }

        Ok(())
    }

    async fn will_disappear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let link_id = resolve_link_id(instance, settings);
        StateRepo::unregister_button(&link_id);
        Ok(())
    }

    async fn property_inspector_did_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let link_id = resolve_link_id(instance, settings);

        let streams: Vec<AudioStream> = match AudioService::list_streams() {
            Ok(s) => s,
            Err(e) => {
                log::error!("[ACTION] Failed to get streams: {}", e);
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
                "event": "updateInfo",
                "link_id": link_id,
                "streams": stream_info,
            }))
            .await?;

        Ok(())
    }
}
