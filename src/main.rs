mod actions;
mod audio;
mod constants;
mod display;
mod icons;
mod pipewire;
mod shared_state;
mod state;
mod volume_monitor;

use simplelog::*;
use std::fs::File;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_file = File::create("/tmp/magic-audio-control.log")?;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Stdout,
            ColorChoice::Never,
        ),
        WriteLogger::new(LevelFilter::Info, Config::default(), log_file),
    ])?;

    log::info!("=== Magic Audio Control Plugin STARTING ===");
    log::info!("Registering actions...");

    state::StateRepo::clear_runtime_registrations();

    volume_monitor::start_volume_monitor();

    openaction::register_action(actions::CycleAudioStreamAction).await;
    openaction::register_action(actions::VolumeControlAction).await;

    log::info!("Actions registered, running plugin...");
    openaction::run(std::env::args().collect()).await?;

    Ok(())
}
