use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::constants::{KNOB_TITLE_ALTERNATE_SECS, MASTER_NODE_ID, MIC_NODE_ID};
use crate::pipewire::{AudioStream, PipeWireManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioTarget {
    Master,
    Mic,
    Stream(u32),
}

#[derive(Debug, Clone)]
pub struct AudioTargetDisplay {
    pub name: String,
    pub icon_query: String,
    alternate_name: Option<String>,
}

type MprisCache = OnceLock<Mutex<HashMap<String, (Instant, Option<String>)>>>;

static MPRIS_TITLE_CACHE: MprisCache = OnceLock::new();
const MPRIS_CACHE_TTL: Duration = Duration::from_secs(2);

fn get_mpris_title_cache() -> &'static Mutex<HashMap<String, (Instant, Option<String>)>> {
    MPRIS_TITLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn normalize_player_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn player_name_aliases(app_name: &str) -> Vec<String> {
    let normalized = normalize_player_name(app_name);
    let mut aliases = vec![normalized.clone()];

    if normalized.contains("chrome") {
        aliases.extend([
            "chrome".to_string(),
            "googlechrome".to_string(),
            "chromium".to_string(),
        ]);
    }

    if normalized.contains("chromium") {
        aliases.extend(["chromium".to_string(), "chrome".to_string()]);
    }

    if normalized.contains("brave") {
        aliases.extend(["brave".to_string(), "bravebrowser".to_string()]);
    }

    if normalized.contains("edge") {
        aliases.extend(["edge".to_string(), "microsoftedge".to_string()]);
    }

    if normalized.contains("firefox") {
        aliases.push("firefox".to_string());
    }

    if normalized.contains("vlc") {
        aliases.extend(["vlc".to_string(), "vlcmediaplayer".to_string()]);
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

fn is_generic_media_name(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "playback" | "audio stream" | "audio playback" | "output" | "stream"
    )
}

#[allow(clippy::manual_pattern_char_comparison)]
fn parse_mpris_bus_names(output: &str) -> Vec<String> {
    let mut names = Vec::new();
    let marker = "org.mpris.MediaPlayer2.";
    let mut search_from = 0;

    while let Some(relative_start) = output[search_from..].find(marker) {
        let start = search_from + relative_start;
        let rest = &output[start..];
        let end = rest
            .find(|ch| matches!(ch, '\'' | '"' | ',' | ']' | ')' | ' ' | '\n' | '\r'))
            .unwrap_or(rest.len());
        let name = &rest[..end];

        if !names.iter().any(|existing| existing == name) {
            names.push(name.to_string());
        }

        search_from = start + marker.len();
    }

    names
}

fn gdbus_call(args: &[&str]) -> Option<String> {
    let output = Command::new("gdbus").args(args).output().ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn list_mpris_players() -> Vec<String> {
    gdbus_call(&[
        "call",
        "--session",
        "--dest",
        "org.freedesktop.DBus",
        "--object-path",
        "/org/freedesktop/DBus",
        "--method",
        "org.freedesktop.DBus.ListNames",
    ])
    .map(|output| parse_mpris_bus_names(&output))
    .unwrap_or_default()
}

fn player_matches_app(player_name: &str, aliases: &[String]) -> bool {
    let normalized = normalize_player_name(player_name);

    aliases.iter().any(|alias| normalized.contains(alias))
}

fn extract_variant_string(output: &str) -> Option<String> {
    for marker in ["<'", "<\""] {
        if let Some(start) = output.find(marker) {
            let quote = marker.chars().last()?;
            let value_start = start + marker.len();
            let value_end = output[value_start..].find(quote)?;
            let value = output[value_start..value_start + value_end].trim();

            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

fn player_playback_status(player_name: &str) -> Option<String> {
    gdbus_call(&[
        "call",
        "--session",
        "--dest",
        player_name,
        "--object-path",
        "/org/mpris/MediaPlayer2",
        "--method",
        "org.freedesktop.DBus.Properties.Get",
        "org.mpris.MediaPlayer2.Player",
        "PlaybackStatus",
    ])
    .and_then(|output| extract_variant_string(&output))
}

fn player_title(player_name: &str) -> Option<String> {
    let output = gdbus_call(&[
        "call",
        "--session",
        "--dest",
        player_name,
        "--object-path",
        "/org/mpris/MediaPlayer2",
        "--method",
        "org.freedesktop.DBus.Properties.Get",
        "org.mpris.MediaPlayer2.Player",
        "Metadata",
    ])?;

    let title_pos = output.find("xesam:title")?;
    extract_variant_string(&output[title_pos..])
}

fn query_mpris_title(app_name: &str) -> Option<String> {
    let aliases = player_name_aliases(app_name);
    let players = list_mpris_players();

    let mut playing_titles = Vec::new();
    let mut paused_titles = Vec::new();
    let mut fallback_titles = Vec::new();

    for player in players {
        if !player_matches_app(&player, &aliases) {
            continue;
        }

        let Some(title) = player_title(&player) else {
            continue;
        };

        match player_playback_status(&player).as_deref() {
            Some("Playing") => playing_titles.push(title),
            Some("Paused") => paused_titles.push(title),
            _ => fallback_titles.push(title),
        }
    }

    playing_titles
        .into_iter()
        .next()
        .or_else(|| paused_titles.into_iter().next())
        .or_else(|| fallback_titles.into_iter().next())
}

fn resolve_mpris_title(app_name: &str) -> Option<String> {
    let cache = get_mpris_title_cache();

    if let Ok(cache_read) = cache.lock() {
        if let Some((cached_at, cached)) = cache_read.get(app_name) {
            if cached_at.elapsed() < MPRIS_CACHE_TTL {
                return cached.clone();
            }
        }
    }

    let title = query_mpris_title(app_name);

    if let Ok(mut cache_write) = cache.lock() {
        cache_write.insert(app_name.to_string(), (Instant::now(), title.clone()));
    }

    title
}

impl AudioTargetDisplay {
    pub fn knob_title(&self) -> String {
        if let Some(alternate_name) = &self.alternate_name {
            let elapsed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if (elapsed / KNOB_TITLE_ALTERNATE_SECS) % 2 == 1 {
                return alternate_name.clone();
            }
        }

        self.name.clone()
    }
}

impl AudioTarget {
    pub fn from_node_id(node_id: u32) -> Self {
        match node_id {
            MASTER_NODE_ID => Self::Master,
            MIC_NODE_ID => Self::Mic,
            _ => Self::Stream(node_id),
        }
    }

    pub fn display_name(self, fallback: &str) -> String {
        match self {
            Self::Master => "Master".to_string(),
            Self::Mic => "Mic".to_string(),
            Self::Stream(_) => fallback.to_string(),
        }
    }

    pub fn icon_query(self, app_name: Option<&str>, fallback_name: &str) -> String {
        match self {
            Self::Master => "mdi:volume-high".to_string(),
            Self::Mic => "mdi:microphone".to_string(),
            Self::Stream(_) => app_name.unwrap_or(fallback_name).to_string(),
        }
    }

    pub fn display_info(
        self,
        name: &str,
        app_name: Option<&str>,
        media_name: Option<&str>,
    ) -> AudioTargetDisplay {
        let primary_name = self.display_name(app_name.unwrap_or(name));
        let alternate_name = match self {
            Self::Stream(_) => media_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .filter(|value| !is_generic_media_name(value))
                .filter(|value| !value.eq_ignore_ascii_case(primary_name.trim()))
                .map(str::to_string)
                .or_else(|| app_name.and_then(resolve_mpris_title))
                .filter(|value| !value.eq_ignore_ascii_case(primary_name.trim()))
                .filter(|value| !is_generic_media_name(value)),
            _ => None,
        };

        AudioTargetDisplay {
            name: primary_name,
            icon_query: self.icon_query(app_name, name),
            alternate_name,
        }
    }
}

pub struct AudioService;

impl AudioService {
    pub fn list_streams() -> Result<Vec<AudioStream>> {
        PipeWireManager::new()?.get_active_streams()
    }

    pub fn most_recent_stream() -> Result<Option<AudioStream>> {
        let streams = Self::list_streams()?;
        Ok(streams.into_iter().max_by_key(|stream| stream.node_id))
    }

    pub fn get_volume(target: AudioTarget) -> Result<u32> {
        match target {
            AudioTarget::Master => PipeWireManager::get_master_volume(),
            AudioTarget::Mic => PipeWireManager::get_mic_volume(),
            AudioTarget::Stream(node_id) => PipeWireManager::get_stream_volume(node_id),
        }
    }

    pub fn set_volume(target: AudioTarget, volume: u32) -> Result<()> {
        match target {
            AudioTarget::Master => PipeWireManager::set_master_volume(volume),
            AudioTarget::Mic => PipeWireManager::set_mic_volume(volume),
            AudioTarget::Stream(node_id) => PipeWireManager::set_stream_volume(node_id, volume),
        }
    }

    pub fn adjust_volume(target: AudioTarget, delta_percent: i32) -> Result<u32> {
        match target {
            AudioTarget::Master => PipeWireManager::adjust_master_volume(delta_percent),
            AudioTarget::Mic => PipeWireManager::adjust_mic_volume(delta_percent),
            AudioTarget::Stream(node_id) => {
                PipeWireManager::adjust_stream_volume(node_id, delta_percent)
            }
        }
    }

    pub fn get_mute(target: AudioTarget) -> Result<bool> {
        match target {
            AudioTarget::Master => PipeWireManager::get_master_mute(),
            AudioTarget::Mic => PipeWireManager::get_mic_mute(),
            AudioTarget::Stream(node_id) => PipeWireManager::get_stream_mute(node_id),
        }
    }

    pub fn toggle_mute(target: AudioTarget) -> Result<bool> {
        match target {
            AudioTarget::Master => PipeWireManager::toggle_master_mute(),
            AudioTarget::Mic => PipeWireManager::toggle_mic_mute(),
            AudioTarget::Stream(node_id) => PipeWireManager::toggle_stream_mute(node_id),
        }
    }
}
