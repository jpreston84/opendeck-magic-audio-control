use crate::actions::{assign_stream_to_button, CycleAudioStreamSettings};
use crate::audio::{AudioService, AudioTarget};
use crate::constants::{COOLDOWN_SECS, POLL_INTERVAL_MS};
use crate::display::{
    blank_button_image, blank_knob_image, format_knob_label, knob_image, update_button_instances,
};
use crate::pipewire::AudioStream;
use crate::state::{StateRepo, StreamSelection};
use openaction::get_instance;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

type VolumeState = HashMap<u32, (u32, bool, String)>;
type VolumeCache = OnceLock<Arc<RwLock<VolumeState>>>;
type CooldownMap = HashMap<String, Instant>;
type CooldownCache = OnceLock<Arc<RwLock<CooldownMap>>>;
type KnownStreams = OnceLock<Arc<RwLock<HashSet<u32>>>>;
type StreamDiscoveryInit = OnceLock<Arc<RwLock<bool>>>;

static VOLUME_CACHE: VolumeCache = VolumeCache::new();
static COOLDOWN: CooldownCache = CooldownCache::new();
static KNOWN_STREAMS: KnownStreams = KnownStreams::new();
static STREAM_DISCOVERY_INITIALIZED: StreamDiscoveryInit = StreamDiscoveryInit::new();

fn get_cache() -> Arc<RwLock<VolumeState>> {
    VOLUME_CACHE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

fn get_cooldown() -> Arc<RwLock<CooldownMap>> {
    COOLDOWN
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

fn get_known_streams() -> Arc<RwLock<HashSet<u32>>> {
    KNOWN_STREAMS
        .get_or_init(|| Arc::new(RwLock::new(HashSet::new())))
        .clone()
}

fn get_stream_discovery_initialized() -> Arc<RwLock<bool>> {
    STREAM_DISCOVERY_INITIALIZED
        .get_or_init(|| Arc::new(RwLock::new(false)))
        .clone()
}

async fn blank_linked_button(link_id: &str, selection: &StreamSelection) {
    let blank_button = blank_button_image();
    let blank_knob = blank_knob_image();
    let button_instance_ids: Vec<String> = StateRepo::get_all_buttons()
        .into_iter()
        .filter(|button| button.link_id == link_id)
        .map(|button| button.instance_id)
        .collect();

    for instance_id in &button_instance_ids {
        if let Some(instance) = get_instance(instance_id.clone()).await {
            let blank_settings = CycleAudioStreamSettings {
                link_id: Some(link_id.to_string()),
                current_node_id: Some(selection.node_id),
                current_name: Some(selection.name.clone()),
                current_app_name: selection.app_name.clone(),
                is_blank: true,
            };

            instance.set_settings(&blank_settings).await.ok();
            instance
                .set_image(Some(blank_button.clone()), None)
                .await
                .ok();
        }
    }

    for knob_instance_id in StateRepo::get_knobs_for_button(link_id) {
        if let Some(knob) = get_instance(knob_instance_id).await {
            knob.set_image(Some(blank_knob.clone()), None).await.ok();
            knob.set_title(None::<String>, None).await.ok();
        }
    }

    StateRepo::clear_selection(link_id);
    StateRepo::update_button_name(link_id, None);
    StateRepo::update_button_icon(link_id, blank_button).await;

    let cache = get_cache();
    let mut cache_write = cache.write().await;
    cache_write.remove(&selection.node_id);
}

pub fn start_volume_monitor() {
    tokio::spawn(async {
        let mut interval = interval(Duration::from_millis(POLL_INTERVAL_MS));

        loop {
            interval.tick().await;

            let buttons = StateRepo::get_all_buttons();
            let knobs = StateRepo::get_all_knobs();
            if knobs.is_empty() && buttons.is_empty() {
                continue;
            }

            let cache = get_cache();
            let cooldown = get_cooldown();
            let now = Instant::now();
            let active_streams: Vec<AudioStream> = match AudioService::list_streams() {
                Ok(streams) => streams,
                Err(e) => {
                    log::error!("[MONITOR] Failed to list streams: {}", e);
                    continue;
                }
            };
            let active_stream_ids = active_streams
                .iter()
                .map(|stream| stream.node_id)
                .collect::<HashSet<_>>();
            let known_streams = get_known_streams();
            let discovery_initialized = get_stream_discovery_initialized();
            let newly_detected_stream = {
                let initialized = *discovery_initialized.read().await;
                let known = known_streams.read().await;

                if initialized {
                    active_streams
                        .iter()
                        .filter(|stream| !known.contains(&stream.node_id))
                        .max_by_key(|stream| stream.node_id)
                        .cloned()
                } else {
                    None
                }
            };
            {
                let mut known = known_streams.write().await;
                *known = active_stream_ids.clone();
            }
            {
                let mut initialized = discovery_initialized.write().await;
                *initialized = true;
            }
            let mut blanked_buttons = HashSet::new();

            for button in &buttons {
                let Some(selection) = StateRepo::load_selection(&button.link_id) else {
                    continue;
                };

                if !matches!(
                    AudioTarget::from_node_id(selection.node_id),
                    AudioTarget::Stream(_)
                ) {
                    continue;
                }

                if !active_stream_ids.contains(&selection.node_id)
                    && blanked_buttons.insert(button.link_id.clone())
                {
                    log::info!(
                        "[MONITOR] Stream {} disappeared for {}, blanking button",
                        selection.node_id,
                        button.link_id
                    );
                    blank_linked_button(&button.link_id, &selection).await;
                }
            }

            if let Some(stream) = newly_detected_stream {
                let mut blank_buttons: Vec<_> = buttons
                    .iter()
                    .filter(|button| StateRepo::load_selection(&button.link_id).is_none())
                    .cloned()
                    .collect();
                blank_buttons.sort_by(|a, b| a.link_id.cmp(&b.link_id));

                if let Some(button) = blank_buttons.into_iter().next() {
                    if let Some(instance) = get_instance(button.instance_id.clone()).await {
                        log::info!(
                            "[MONITOR] New stream {} detected, assigning blank button {}",
                            stream.node_id,
                            button.link_id
                        );
                        assign_stream_to_button(&instance, &button.link_id, &stream)
                            .await
                            .ok();
                    }
                }
            }

            for knob in knobs {
                let selection = match StateRepo::load_selection(&knob.linked_button) {
                    Some(s) => s,
                    None => {
                        if let Some(instance) = get_instance(knob.instance_id.clone()).await {
                            instance
                                .set_image(Some(blank_knob_image()), None)
                                .await
                                .ok();
                            instance.set_title(None::<String>, None).await.ok();
                        }
                        continue;
                    }
                };

                let target = AudioTarget::from_node_id(selection.node_id);

                if blanked_buttons.contains(&knob.linked_button) {
                    continue;
                }

                {
                    let cooldown_read = cooldown.read().await;
                    if let Some(last_touch) = cooldown_read.get(&knob.instance_id) {
                        if now.duration_since(*last_touch) < Duration::from_secs(COOLDOWN_SECS) {
                            continue;
                        }
                    }
                }

                let current_vol = match AudioService::get_volume(target) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let is_muted = AudioService::get_mute(target).unwrap_or(false);

                let display_info = target.display_info(
                    &selection.name,
                    selection.app_name.as_deref(),
                    selection.media_name.as_deref(),
                );
                let knob_title = display_info.knob_title();

                let last_state = {
                    let cache_read = cache.read().await;
                    cache_read.get(&selection.node_id).cloned()
                };

                if last_state != Some((current_vol, is_muted, knob_title.clone())) {
                    {
                        let mut cache_write = cache.write().await;
                        cache_write.insert(
                            selection.node_id,
                            (current_vol, is_muted, knob_title.clone()),
                        );
                    }

                    let display = knob_image(&display_info.name, current_vol, is_muted);

                    if let Some(instance) = get_instance(knob.instance_id.clone()).await {
                        instance.set_image(Some(display), None).await.ok();
                        instance
                            .set_title(Some(format_knob_label(&knob_title)), None)
                            .await
                            .ok();
                    }

                    let button_instance_ids: Vec<String> = StateRepo::get_all_buttons()
                        .into_iter()
                        .filter(|button| button.link_id == knob.linked_button)
                        .map(|button| button.instance_id)
                        .collect();
                    if let Some(icon) = update_button_instances(
                        button_instance_ids,
                        &display_info.icon_query,
                        is_muted,
                    )
                    .await
                    {
                        StateRepo::update_button_icon(&knob.linked_button, icon).await;
                    }
                }
            }
        }
    });
}

pub async fn update_cached_volume(node_id: u32, volume: u32, title: &str) {
    let cache = get_cache();
    let mut cache_write = cache.write().await;
    let muted = cache_write
        .get(&node_id)
        .map(|(_, muted, _)| *muted)
        .unwrap_or(false);
    cache_write.insert(node_id, (volume, muted, title.to_string()));
}

pub async fn update_cached_display_state(node_id: u32, volume: u32, muted: bool, title: &str) {
    let cache = get_cache();
    let mut cache_write = cache.write().await;
    cache_write.insert(node_id, (volume, muted, title.to_string()));
}

pub async fn touch_knob(instance_id: &str) {
    let cooldown = get_cooldown();
    let mut cooldown_write = cooldown.write().await;
    cooldown_write.insert(instance_id.to_string(), Instant::now());
}
