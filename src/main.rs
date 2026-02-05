//! Gander - A terminal markdown viewer with image support.
//!
//! # Usage
//!
//! ```bash
//! gander README.md
//! gander --watch README.md
//! gander --no-toc README.md
//! ```

use std::fs;
use std::path::{Path, PathBuf};

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

    /// Save current command-line flags as defaults in .ganderrc
    #[arg(long)]
    save: bool,

    /// Clear saved defaults in .ganderrc
    #[arg(long)]
    clear: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ConfigFlags {
    watch: bool,
    no_toc: bool,
    toc: bool,
    perf: bool,
    force_half_cell: bool,
    theme: Option<ThemeMode>,
    render_debug_log: Option<PathBuf>,
}

impl ConfigFlags {
    fn union(&self, other: &Self) -> Self {
        Self {
            watch: self.watch || other.watch,
            no_toc: self.no_toc || other.no_toc,
            toc: self.toc || other.toc,
            perf: self.perf || other.perf,
            force_half_cell: self.force_half_cell || other.force_half_cell,
            theme: other.theme.or(self.theme),
            render_debug_log: other
                .render_debug_log
                .clone()
                .or_else(|| self.render_debug_log.clone()),
        }
    }
}

fn global_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("gander").join("config");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("gander")
                .join("config");
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("gander").join("config");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config").join("gander").join("config");
        }
    }

    PathBuf::from(".ganderrc")
}

fn local_override_path() -> PathBuf {
    PathBuf::from(".ganderrc")
}

fn load_config_flags(path: &Path) -> Result<ConfigFlags> {
    if !path.exists() {
        return Ok(ConfigFlags::default());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config {}", path.display()))?;
    let tokens = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .flat_map(|line| line.split_whitespace().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    Ok(parse_flag_tokens(&tokens))
}

fn save_config_flags(path: &Path, flags: &ConfigFlags) -> Result<()> {
    let mut lines = Vec::new();
    lines.push("# gander defaults (saved with --save)".to_string());
    if flags.watch {
        lines.push("--watch".to_string());
    }
    if flags.no_toc {
        lines.push("--no-toc".to_string());
    }
    if flags.toc {
        lines.push("--toc".to_string());
    }
    if let Some(theme) = flags.theme {
        let theme_str = match theme {
            ThemeMode::Auto => "auto",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        };
        lines.push(format!("--theme {}", theme_str));
    }
    if flags.perf {
        lines.push("--perf".to_string());
    }
    if let Some(path) = &flags.render_debug_log {
        lines.push(format!("--render-debug-log {}", path.display()));
    }
    if flags.force_half_cell {
        lines.push("--force-half-cell".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .with_context(|| format!("Failed to write config {}", path.display()))
}

fn clear_config_flags(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

fn parse_flag_tokens(tokens: &[String]) -> ConfigFlags {
    let mut flags = ConfigFlags::default();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        if token == "--watch" {
            flags.watch = true;
        } else if token == "--no-toc" {
            flags.no_toc = true;
        } else if token == "--toc" {
            flags.toc = true;
        } else if token == "--perf" {
            flags.perf = true;
        } else if token == "--force-half-cell" {
            flags.force_half_cell = true;
        } else if token == "--theme" {
            if let Some(next) = tokens.get(i + 1) {
                flags.theme = parse_theme(next);
                i += 1;
            }
        } else if let Some(value) = token.strip_prefix("--theme=") {
            flags.theme = parse_theme(value);
        } else if token == "--render-debug-log" {
            if let Some(next) = tokens.get(i + 1) {
                flags.render_debug_log = Some(PathBuf::from(next));
                i += 1;
            }
        } else if let Some(value) = token.strip_prefix("--render-debug-log=") {
            flags.render_debug_log = Some(PathBuf::from(value));
        }
        i += 1;
    }
    flags
}

fn parse_theme(s: &str) -> Option<ThemeMode> {
    match s {
        "auto" => Some(ThemeMode::Auto),
        "light" => Some(ThemeMode::Light),
        "dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .init();

    let raw_args = std::env::args().collect::<Vec<_>>();
    let cli = Cli::parse();
    let global_path = global_config_path();
    let local_path = local_override_path();
    let cli_flags = parse_flag_tokens(&raw_args);

    if cli.clear {
        clear_config_flags(&global_path)?;
    }
    if cli.save {
        save_config_flags(&global_path, &cli_flags)?;
    }

    let file_flags = if cli.clear {
        ConfigFlags::default()
    } else {
        let global_flags = load_config_flags(&global_path)?;
        let local_flags = load_config_flags(&local_path)?;
        global_flags.union(&local_flags)
    };
    let effective = file_flags.union(&cli_flags);

    perf::set_enabled(effective.perf);
    let render_debug_log_path = effective
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

    match effective.theme.unwrap_or(ThemeMode::Auto) {
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
        .with_watch(effective.watch)
        .with_toc_visible(effective.toc && !effective.no_toc)
        .with_force_half_cell(effective.force_half_cell)
        .with_config_paths(
            Some(global_path.clone()),
            if local_path.exists() {
                Some(local_path.clone())
            } else {
                None
            },
        );

    app.run().context("Application error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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

    #[test]
    fn test_save_and_clear_flags_parse() {
        let cli = Cli::try_parse_from(["gander", "--save", "--clear", "README.md"]).unwrap();
        assert!(cli.save);
        assert!(cli.clear);
    }

    #[test]
    fn test_parse_flag_tokens_extracts_known_flags() {
        let args = vec![
            "gander".to_string(),
            "--watch".to_string(),
            "--toc".to_string(),
            "--theme".to_string(),
            "dark".to_string(),
            "--render-debug-log=render.log".to_string(),
            "--force-half-cell".to_string(),
            "README.md".to_string(),
        ];
        let flags = parse_flag_tokens(&args);
        assert!(flags.watch);
        assert!(flags.toc);
        assert_eq!(flags.theme, Some(ThemeMode::Dark));
        assert_eq!(flags.render_debug_log, Some(PathBuf::from("render.log")));
        assert!(flags.force_half_cell);
    }

    #[test]
    fn test_config_union_merges_cli_over_file_for_options() {
        let file = ConfigFlags {
            watch: true,
            theme: Some(ThemeMode::Light),
            ..ConfigFlags::default()
        };
        let cli = ConfigFlags {
            toc: true,
            theme: Some(ThemeMode::Dark),
            ..ConfigFlags::default()
        };
        let merged = file.union(&cli);
        assert!(merged.watch);
        assert!(merged.toc);
        assert_eq!(merged.theme, Some(ThemeMode::Dark));
    }

    #[test]
    fn test_save_load_and_clear_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".ganderrc");
        let flags = ConfigFlags {
            watch: true,
            toc: true,
            perf: true,
            force_half_cell: true,
            theme: Some(ThemeMode::Dark),
            render_debug_log: Some(PathBuf::from("render.log")),
            ..ConfigFlags::default()
        };

        save_config_flags(&path, &flags).unwrap();
        let loaded = load_config_flags(&path).unwrap();
        assert_eq!(loaded.watch, true);
        assert_eq!(loaded.toc, true);
        assert_eq!(loaded.perf, true);
        assert_eq!(loaded.force_half_cell, true);
        assert_eq!(loaded.theme, Some(ThemeMode::Dark));
        assert_eq!(loaded.render_debug_log, Some(PathBuf::from("render.log")));

        clear_config_flags(&path).unwrap();
        assert!(!path.exists());
    }
}
