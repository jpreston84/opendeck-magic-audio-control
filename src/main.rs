mod actions;
mod pipewire;

use simplelog::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    TermLogger::init(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Stdout,
        ColorChoice::Never,
    )?;

    openaction::register_action(actions::CycleAudioStreamAction).await;
    openaction::run(std::env::args().collect()).await?;

    Ok(())
}
