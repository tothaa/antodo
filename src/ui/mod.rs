//! Iced presentation and input handling. Domain behavior lives exclusively in core.
mod app;
mod datetime;
mod editor;
mod markdown_editor;
mod views;

pub mod preview;

/// Handle desktop startup arguments before launching Iced.
pub fn run() -> iced::Result {
    let argument = std::env::args_os().nth(1);
    match argument.as_ref().and_then(|value| value.to_str()) {
        Some("--version" | "-V") => {
            println!("antodo {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h") => {
            println!(
                "antodo — a local task planner\n\nUsage: antodo [FILE.antodo]\n\nWithout a file, reopen the last workspace.\n\n  -h, --help       Show this help\n  -V, --version    Show the version"
            );
            Ok(())
        }
        _ => app::run(),
    }
}

mod table;

mod mcp;

mod llm;

mod icon;
mod notes;
mod task_interactions;
