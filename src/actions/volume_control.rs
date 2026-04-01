use crate::audio::{AudioService, AudioTarget};
use crate::constants::DEBOUNCE_MS;
use crate::display::{blank_knob_image, format_knob_label, update_button_instances};
use crate::state::StateRepo;
use crate::volume_monitor::{touch_knob, update_cached_display_state, update_cached_volume};
use openaction::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

static ACCUMULATED_TICKS: AtomicI32 = AtomicI32::new(0);
static LAST_ROTATION: std::sync::OnceLock<std::sync::Mutex<Instant>> = std::sync::OnceLock::new();

fn get_last_rotation() -> &'static std::sync::Mutex<Instant> {
    LAST_ROTATION.get_or_init(|| std::sync::Mutex::new(Instant::now()))
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct VolumeControlSettings {
    pub link_id: Option<String>,
    pub current_volume: u32,
    pub pre_mute_volume: u32,
    pub increment: u32,
}

impl Default for VolumeControlSettings {
    fn default() -> Self {
        Self {
            link_id: None,
            current_volume: 0,
            pre_mute_volume: 0,
            increment: 5,
        }
    }
}

pub struct VolumeControlAction;

#[async_trait]
impl Action for VolumeControlAction {
    const UUID: &'static str = "net.jpreston.opendeck.audio.volume-control";
    type Settings = VolumeControlSettings;

    async fn dial_rotate(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
        ticks: i16,
        _pressed: bool,
    ) -> OpenActionResult<()> {
        touch_knob(&instance.instance_id).await;

        let link_id = settings.link_id.as_ref().filter(|s| !s.is_empty());
        let Some(link_id) = link_id else {
            return Ok(());
        };

        StateRepo::lock_button(link_id).await;

        let selection = StateRepo::load_selection(link_id);
        let Some(selection) = selection else {
            return Ok(());
        };

        let target = AudioTarget::from_node_id(selection.node_id);

        let is_muted = AudioService::get_mute(target).unwrap_or(false);

        if is_muted {
            return Ok(());
        }

        let current_volume = AudioService::get_volume(target).unwrap_or(100);

        let now = Instant::now();
        let should_process = {
            let mut last = get_last_rotation().lock().unwrap();
            let elapsed = now.duration_since(*last);
            *last = now;
            elapsed.as_millis() > DEBOUNCE_MS as u128
        };

        let accumulated = ACCUMULATED_TICKS.fetch_add(ticks as i32, Ordering::Relaxed);
        let total_ticks = accumulated + ticks as i32;

        if !should_process {
            return Ok(());
        }

        ACCUMULATED_TICKS.store(0, Ordering::Relaxed);

        let increment = settings.increment.clamp(1, 20) as i32;
        let delta = total_ticks * increment;

        let new_volume = AudioService::adjust_volume(target, delta).unwrap_or(current_volume);

        let clamped_volume = new_volume.clamp(1, 100);
        if clamped_volume != new_volume {
            AudioService::set_volume(target, clamped_volume).ok();
        }

        let display_info = target.display_info(
            &selection.name,
            selection.app_name.as_deref(),
            selection.media_name.as_deref(),
        );
        let display = crate::display::knob_image(&display_info.name, clamped_volume, false);
        instance.set_image(Some(display), None).await?;
        let knob_title = display_info.knob_title();
        instance
            .set_title(Some(format_knob_label(&knob_title)), None)
            .await?;
        update_cached_volume(selection.node_id, clamped_volume, &knob_title).await;

        Ok(())
    }

    async fn dial_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        touch_knob(&instance.instance_id).await;
        let link_id = settings.link_id.as_ref().filter(|s| !s.is_empty());
        let Some(link_id) = link_id else {
            instance.show_alert().await?;
            return Ok(());
        };

        StateRepo::lock_button(link_id).await;

        let selection = StateRepo::load_selection(link_id);
        let Some(selection) = selection else {
            instance.show_alert().await?;
            return Ok(());
        };

        let target = AudioTarget::from_node_id(selection.node_id);

        let muted = AudioService::toggle_mute(target).unwrap_or(false);
        let volume = AudioService::get_volume(target).unwrap_or(100);

        let new_settings = VolumeControlSettings {
            link_id: settings.link_id.clone(),
            current_volume: volume,
            pre_mute_volume: settings.pre_mute_volume,
            increment: settings.increment,
        };
        instance.set_settings(&new_settings).await?;

        let display_info = target.display_info(
            &selection.name,
            selection.app_name.as_deref(),
            selection.media_name.as_deref(),
        );
        let display = crate::display::knob_image(&display_info.name, volume, muted);
        instance.set_image(Some(display), None).await?;
        let knob_title = display_info.knob_title();
        instance
            .set_title(Some(format_knob_label(&knob_title)), None)
            .await?;
        update_cached_display_state(selection.node_id, volume, muted, &knob_title).await;

        let button_instance_ids: Vec<String> = StateRepo::get_all_buttons()
            .into_iter()
            .filter(|button| button.link_id == *link_id)
            .map(|button| button.instance_id)
            .collect();
        if let Some(icon) =
            update_button_instances(button_instance_ids, &display_info.icon_query, muted).await
        {
            StateRepo::update_button_icon(link_id, icon).await;
        }

        log::info!("[KNOB] Mute: muted={}, volume={}", muted, volume);

        Ok(())
    }

    async fn will_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        if let Some(ref link_id) = settings.link_id {
            if !link_id.is_empty() {
                StateRepo::register_knob(&instance.instance_id, link_id);
                if let Some(selection) = StateRepo::load_selection(link_id) {
                    let target = AudioTarget::from_node_id(selection.node_id);
                    let vol = AudioService::get_volume(target)
                        .unwrap_or(settings.current_volume)
                        .max(1);
                    let is_muted = AudioService::get_mute(target).unwrap_or(false);
                    let display_info = target.display_info(
                        &selection.name,
                        selection.app_name.as_deref(),
                        selection.media_name.as_deref(),
                    );
                    let display = crate::display::knob_image(&display_info.name, vol, is_muted);
                    let knob_title = display_info.knob_title();
                    instance.set_image(Some(display), None).await?;
                    instance
                        .set_title(Some(format_knob_label(&knob_title)), None)
                        .await?;
                    return Ok(());
                }
            }
        }
        instance.set_image(Some(blank_knob_image()), None).await?;
        instance.set_title(None::<String>, None).await?;
        Ok(())
    }

    async fn will_disappear(
        &self,
        instance: &Instance,
        _settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        StateRepo::unregister_knob(&instance.instance_id);
        Ok(())
    }

    async fn property_inspector_did_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let buttons = StateRepo::get_all_buttons();

        let button_list: Vec<serde_json::Value> = buttons
            .iter()
            .map(|b| {
                serde_json::json!({
                    "link_id": b.link_id,
                    "name": b.current_name.as_deref().unwrap_or("Unused"),
                })
            })
            .collect();

        instance
            .send_to_property_inspector(serde_json::json!({
                "event": "updateButtons",
                "buttons": button_list,
                "current_link_id": settings.link_id,
            }))
            .await?;

        Ok(())
    }
}
