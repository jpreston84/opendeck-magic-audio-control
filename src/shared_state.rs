use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StreamSelection {
    pub node_id: u32,
    pub name: String,
    pub app_name: Option<String>,
    pub media_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonInfo {
    pub link_id: String,
    pub instance_id: String,
    pub current_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnobInfo {
    pub instance_id: String,
    pub linked_button: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharedState {
    pub selections: HashMap<String, StreamSelection>,
    pub buttons: HashMap<String, ButtonInfo>,
    pub knobs: HashMap<String, KnobInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct ButtonLockState {
    pub is_unlocked: bool,
    pub press_start: Option<Instant>,
    pub unlock_expires: Option<Instant>,
    pub current_icon: Option<String>,
}

static BUTTON_LOCK_STATES: std::sync::OnceLock<Arc<RwLock<HashMap<String, ButtonLockState>>>> =
    std::sync::OnceLock::new();

fn get_lock_states() -> Arc<RwLock<HashMap<String, ButtonLockState>>> {
    BUTTON_LOCK_STATES
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

pub async fn get_button_lock_state(link_id: &str) -> ButtonLockState {
    let states = get_lock_states();
    let read = states.read().await;
    read.get(link_id).cloned().unwrap_or_default()
}

pub async fn record_press_start(link_id: &str) {
    let states = get_lock_states();
    let mut write = states.write().await;
    let entry = write
        .entry(link_id.to_string())
        .or_insert_with(ButtonLockState::default);
    entry.press_start = Some(Instant::now());
}

pub async fn clear_press_start(link_id: &str) -> Option<Instant> {
    let states = get_lock_states();
    let mut write = states.write().await;
    if let Some(entry) = write.get_mut(link_id) {
        let start = entry.press_start.take();
        entry.press_start = None;
        start
    } else {
        None
    }
}

pub async fn unlock_button(link_id: &str) {
    let states = get_lock_states();
    let mut write = states.write().await;
    let entry = write
        .entry(link_id.to_string())
        .or_insert_with(ButtonLockState::default);
    entry.is_unlocked = true;
    entry.unlock_expires = Some(Instant::now() + std::time::Duration::from_secs(5));
}

pub async fn lock_button(link_id: &str) {
    let states = get_lock_states();
    let mut write = states.write().await;
    if let Some(entry) = write.get_mut(link_id) {
        entry.is_unlocked = false;
        entry.unlock_expires = None;
    }
}

pub async fn refresh_unlock_timer(link_id: &str) {
    let states = get_lock_states();
    let mut write = states.write().await;
    if let Some(entry) = write.get_mut(link_id) {
        if entry.is_unlocked {
            entry.unlock_expires = Some(Instant::now() + std::time::Duration::from_secs(5));
        }
    }
}

pub async fn is_button_unlocked(link_id: &str) -> bool {
    let states = get_lock_states();
    let read = states.read().await;
    read.get(link_id).map(|s| s.is_unlocked).unwrap_or(false)
}

pub async fn update_button_icon(link_id: &str, icon: String) {
    let states = get_lock_states();
    let mut write = states.write().await;
    if let Some(entry) = write.get_mut(link_id) {
        entry.current_icon = Some(icon);
    }
}

pub async fn get_button_icon(link_id: &str) -> Option<String> {
    let states = get_lock_states();
    let read = states.read().await;
    read.get(link_id).and_then(|s| s.current_icon.clone())
}

fn state_path() -> PathBuf {
    std::env::temp_dir().join("opendeck-audio-streams.json")
}

fn load_all() -> SharedState {
    let path = state_path();
    let mut file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return SharedState::default(),
    };

    let mut json = String::new();
    if file.read_to_string(&mut json).is_err() {
        return SharedState::default();
    }

    serde_json::from_str(&json).unwrap_or_default()
}

fn save_all(state: &SharedState) {
    let path = state_path();
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = fs::File::create(&path).and_then(|mut f| f.write_all(json.as_bytes()));
    }
}

pub fn save_selection(link_id: &str, selection: &StreamSelection) {
    let mut state = load_all();
    state
        .selections
        .insert(link_id.to_string(), selection.clone());
    save_all(&state);
}

pub fn load_selection(link_id: &str) -> Option<StreamSelection> {
    let state = load_all();
    state.selections.get(link_id).cloned()
}

pub fn clear_selection(link_id: &str) {
    let mut state = load_all();
    state.selections.remove(link_id);
    save_all(&state);
}

pub fn register_button(link_id: &str, instance_id: &str, current_name: Option<&str>) {
    let mut state = load_all();
    state.buttons.insert(
        link_id.to_string(),
        ButtonInfo {
            link_id: link_id.to_string(),
            instance_id: instance_id.to_string(),
            current_name: current_name.map(String::from),
        },
    );
    save_all(&state);
}

pub fn update_button_name(link_id: &str, current_name: Option<&str>) {
    let mut state = load_all();
    if let Some(button) = state.buttons.get_mut(link_id) {
        button.current_name = current_name.map(String::from);
        save_all(&state);
    }
}

pub fn unregister_button(link_id: &str) {
    let mut state = load_all();
    state.buttons.remove(link_id);
    save_all(&state);
}

pub fn get_all_buttons() -> Vec<ButtonInfo> {
    let state = load_all();
    state.buttons.values().cloned().collect()
}

pub fn find_button_link_id(instance_id: &str) -> Option<String> {
    let state = load_all();
    state
        .buttons
        .values()
        .find(|button| button.instance_id == instance_id)
        .map(|button| button.link_id.clone())
}

pub fn clear_runtime_registrations() {
    let mut state = load_all();
    state.buttons.clear();
    state.knobs.clear();
    save_all(&state);
}

pub fn register_knob(instance_id: &str, linked_button: &str) {
    let mut state = load_all();
    state.knobs.insert(
        instance_id.to_string(),
        KnobInfo {
            instance_id: instance_id.to_string(),
            linked_button: linked_button.to_string(),
        },
    );
    save_all(&state);
}

pub fn unregister_knob(instance_id: &str) {
    let mut state = load_all();
    state.knobs.remove(instance_id);
    save_all(&state);
}

pub fn get_knobs_for_button(link_id: &str) -> Vec<String> {
    let state = load_all();
    state
        .knobs
        .values()
        .filter(|k| k.linked_button == link_id)
        .map(|k| k.instance_id.clone())
        .collect()
}

pub fn get_all_knobs() -> Vec<KnobInfo> {
    let state = load_all();
    state.knobs.values().cloned().collect()
}
