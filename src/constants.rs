use std::time::Duration;

pub const MASTER_NODE_ID: u32 = 0xFFFF_FFFF;
pub const MIC_NODE_ID: u32 = 0xFFFF_FFFE;

pub const LONG_PRESS_DURATION: Duration = Duration::from_secs(1);
pub const OSCILLATE_INTERVAL: Duration = Duration::from_millis(250);
pub const POLL_INTERVAL_MS: u64 = 2000;
pub const COOLDOWN_SECS: u64 = 3;
pub const DEBOUNCE_MS: u64 = 50;
pub const KNOB_TITLE_ALTERNATE_SECS: u64 = 2;
