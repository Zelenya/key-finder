use anyhow::{bail, Context};
use clap::Parser;
use key_finder::cli::commands;
use key_finder::cli::options::Cli;
use std::process;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{err:#}");
        process::exit(2);
    }
}

fn try_main() -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        bail!(
            "unsupported platform: this app currently supports macOS only. \
to extend support, add target-specific notifier implementations in src/domain/notifier.rs"
        );
    }

    let (config, initial_shortcuts) = Cli::parse()
        .into_runtime_inputs()
        .context("failed to build app configuration from CLI, environment, and SQLite settings")?;

    commands::run(config, initial_shortcuts).context("key finder runtime failed")?;
    Ok(())
}
