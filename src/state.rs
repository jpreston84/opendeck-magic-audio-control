pub use crate::shared_state::{ButtonInfo, ButtonLockState, KnobInfo, StreamSelection};

pub struct StateRepo;

impl StateRepo {
    pub fn save_selection(link_id: &str, selection: &StreamSelection) {
        crate::shared_state::save_selection(link_id, selection);
    }

    pub fn load_selection(link_id: &str) -> Option<StreamSelection> {
        crate::shared_state::load_selection(link_id)
    }

    pub fn clear_selection(link_id: &str) {
        crate::shared_state::clear_selection(link_id);
    }

    pub fn register_button(link_id: &str, instance_id: &str, current_name: Option<&str>) {
        crate::shared_state::register_button(link_id, instance_id, current_name);
    }

    pub fn update_button_name(link_id: &str, current_name: Option<&str>) {
        crate::shared_state::update_button_name(link_id, current_name);
    }

    pub fn unregister_button(link_id: &str) {
        crate::shared_state::unregister_button(link_id);
    }

    pub fn get_all_buttons() -> Vec<ButtonInfo> {
        crate::shared_state::get_all_buttons()
    }

    pub fn find_button_link_id(instance_id: &str) -> Option<String> {
        crate::shared_state::find_button_link_id(instance_id)
    }

    pub fn clear_runtime_registrations() {
        crate::shared_state::clear_runtime_registrations();
    }

    pub fn register_knob(instance_id: &str, linked_button: &str) {
        crate::shared_state::register_knob(instance_id, linked_button);
    }

    pub fn unregister_knob(instance_id: &str) {
        crate::shared_state::unregister_knob(instance_id);
    }

    pub fn get_knobs_for_button(link_id: &str) -> Vec<String> {
        crate::shared_state::get_knobs_for_button(link_id)
    }

    pub fn get_all_knobs() -> Vec<KnobInfo> {
        crate::shared_state::get_all_knobs()
    }

    pub async fn get_button_lock_state(link_id: &str) -> ButtonLockState {
        crate::shared_state::get_button_lock_state(link_id).await
    }

    pub async fn record_press_start(link_id: &str) {
        crate::shared_state::record_press_start(link_id).await;
    }

    pub async fn clear_press_start(link_id: &str) -> Option<std::time::Instant> {
        crate::shared_state::clear_press_start(link_id).await
    }

    pub async fn unlock_button(link_id: &str) {
        crate::shared_state::unlock_button(link_id).await;
    }

    pub async fn lock_button(link_id: &str) {
        crate::shared_state::lock_button(link_id).await;
    }

    pub async fn refresh_unlock_timer(link_id: &str) {
        crate::shared_state::refresh_unlock_timer(link_id).await;
    }

    pub async fn is_button_unlocked(link_id: &str) -> bool {
        crate::shared_state::is_button_unlocked(link_id).await
    }

    pub async fn update_button_icon(link_id: &str, icon: String) {
        crate::shared_state::update_button_icon(link_id, icon).await;
    }

    pub async fn get_button_icon(link_id: &str) -> Option<String> {
        crate::shared_state::get_button_icon(link_id).await
    }
}
