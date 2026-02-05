//! Gander - A terminal markdown viewer with image support.
//!
//! # Usage
//!
//! ```bash
//! gander README.md
//! gander --watch README.md
//! gander --no-toc README.md
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use gander::app::App;
use gander::highlight::{set_background_mode, HighlightBackground};
use gander::perf;

/// A terminal markdown viewer with image support
#[derive(Parser, Debug)]
#[command(name = "gander", version, about, long_about = None)]
struct Cli {
    /// Markdown file to view
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Watch file for changes and auto-reload
    #[arg(short, long)]
    watch: bool,

    /// Hide table of contents sidebar
    #[arg(long)]
    no_toc: bool,

    /// Start with TOC sidebar visible
    #[arg(long)]
    toc: bool,

    /// Force syntax highlight theme background (light or dark)
    #[arg(long, value_enum, default_value = "auto")]
    theme: ThemeMode,

    /// Enable startup performance logging
    #[arg(long)]
    perf: bool,

    /// Write detailed render/image debug events to a file
    #[arg(long, value_name = "PATH")]
    render_debug_log: Option<PathBuf>,

    /// Force image rendering to use half-cell fallback mode
    #[arg(long)]
    force_half_cell: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Auto,
    Light,
    Dark,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let cli = Cli::parse();
    perf::set_enabled(cli.perf);
    let render_debug_log_path = cli
        .render_debug_log
        .clone()
        .or_else(|| std::env::var_os("GANDER_RENDER_DEBUG_LOG").map(PathBuf::from));
    if let Err(err) = perf::set_debug_log_path(render_debug_log_path.as_deref()) {
        eprintln!(
            "[warn] Failed to initialize render debug log {}: {}",
            render_debug_log_path
                .as_ref()
                .map_or_else(|| "<unset>".to_string(), |p| p.display().to_string()),
            err
        );
    }

    match cli.theme {
        ThemeMode::Auto => set_background_mode(None),
        ThemeMode::Light => set_background_mode(Some(HighlightBackground::Light)),
        ThemeMode::Dark => set_background_mode(Some(HighlightBackground::Dark)),
    }

    // Verify file exists
    if !cli.file.exists() {
        anyhow::bail!("File not found: {}", cli.file.display());
    }

    // Run the application
    let mut app = App::new(cli.file)
        .with_watch(cli.watch)
        .with_toc_visible(cli.toc && !cli.no_toc)
        .with_force_half_cell(cli.force_half_cell);

    app.run().context("Application error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_flag_parses() {
        let cli = Cli::try_parse_from(["gander", "--theme", "light", "README.md"]).unwrap();
        assert_eq!(cli.theme, ThemeMode::Light);
    }

    #[test]
    fn test_perf_flag_parses() {
        let cli = Cli::try_parse_from(["gander", "--perf", "README.md"]).unwrap();
        assert!(cli.perf);
    }

    #[test]
    fn test_render_debug_log_flag_parses() {
        let cli = Cli::try_parse_from([
            "gander",
            "--render-debug-log",
            "render.log",
            "README.md",
        ])
        .unwrap();
        assert_eq!(cli.render_debug_log, Some(PathBuf::from("render.log")));
    }

    #[test]
    fn test_force_half_cell_flag_parses() {
        let cli = Cli::try_parse_from(["gander", "--force-half-cell", "README.md"]).unwrap();
        assert!(cli.force_half_cell);
    }
}
