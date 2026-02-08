use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Auto,
    Light,
    Dark,
}

/// Forced image rendering mode.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    /// Kitty graphics protocol
    Kitty,
    /// Sixel graphics
    Sixel,
    /// iTerm2 inline images
    #[value(name = "iterm2")]
    ITerm2,
    /// Unicode half-blocks (universal fallback)
    Halfblock,
}

impl std::fmt::Display for ImageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kitty => write!(f, "Kitty"),
            Self::Sixel => write!(f, "Sixel"),
            Self::ITerm2 => write!(f, "iTerm2"),
            Self::Halfblock => write!(f, "Halfblock"),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigFlags {
    pub watch: bool,
    pub no_toc: bool,
    pub toc: bool,
    pub no_images: bool,
    pub perf: bool,
    pub force_half_cell: bool,
    pub image_mode: Option<ImageMode>,
    pub theme: Option<ThemeMode>,
    pub render_debug_log: Option<PathBuf>,
}

impl ConfigFlags {
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            watch: self.watch || other.watch,
            no_toc: self.no_toc || other.no_toc,
            toc: self.toc || other.toc,
            no_images: self.no_images || other.no_images,
            perf: self.perf || other.perf,
            force_half_cell: self.force_half_cell || other.force_half_cell,
            image_mode: other.image_mode.or(self.image_mode),
            theme: other.theme.or(self.theme),
            render_debug_log: other
                .render_debug_log
                .clone()
                .or_else(|| self.render_debug_log.clone()),
        }
    }
}

pub fn global_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("markless").join("config");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("markless")
                .join("config");
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("markless").join("config");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("markless")
                .join("config");
        }
    }

    PathBuf::from(".marklessrc")
}

pub fn local_override_path() -> PathBuf {
    PathBuf::from(".marklessrc")
}

/// Load configuration flags from a file at the given path.
///
/// # Errors
/// Returns an error if the config file exists but cannot be read.
pub fn load_config_flags(path: &Path) -> Result<ConfigFlags> {
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

/// Save configuration flags to a file at the given path.
///
/// # Errors
/// Returns an error if the config directory cannot be created or the file cannot be written.
pub fn save_config_flags(path: &Path, flags: &ConfigFlags) -> Result<()> {
    let mut lines = Vec::new();
    lines.push("# markless defaults (saved with --save)".to_string());
    if flags.watch {
        lines.push("--watch".to_string());
    }
    if flags.no_toc {
        lines.push("--no-toc".to_string());
    }
    if flags.toc {
        lines.push("--toc".to_string());
    }
    if flags.no_images {
        lines.push("--no-images".to_string());
    }
    if let Some(theme) = flags.theme {
        let theme_str = match theme {
            ThemeMode::Auto => "auto",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        };
        lines.push(format!("--theme {theme_str}"));
    }
    if flags.perf {
        lines.push("--perf".to_string());
    }
    if let Some(path) = &flags.render_debug_log {
        lines.push(format!("--render-debug-log {}", path.display()));
    }
    if let Some(mode) = flags.image_mode {
        let mode_str = match mode {
            ImageMode::Kitty => "kitty",
            ImageMode::Sixel => "sixel",
            ImageMode::ITerm2 => "iterm2",
            ImageMode::Halfblock => "halfblock",
        };
        lines.push(format!("--image-mode {mode_str}"));
    } else if flags.force_half_cell {
        lines.push("--force-half-cell".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .with_context(|| format!("Failed to write config {}", path.display()))
}

/// Remove the config file at the given path if it exists.
///
/// # Errors
/// Returns an error if the file exists but cannot be removed.
pub fn clear_config_flags(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn parse_flag_tokens(tokens: &[String]) -> ConfigFlags {
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
        } else if token == "--no-images" {
            flags.no_images = true;
        } else if token == "--perf" {
            flags.perf = true;
        } else if token == "--force-half-cell" {
            flags.force_half_cell = true;
            flags.image_mode = Some(ImageMode::Halfblock);
        } else if token == "--image-mode" {
            if let Some(next) = tokens.get(i + 1) {
                flags.image_mode = parse_image_mode(next);
                i += 1;
            }
        } else if let Some(value) = token.strip_prefix("--image-mode=") {
            flags.image_mode = parse_image_mode(value);
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

fn parse_image_mode(s: &str) -> Option<ImageMode> {
    match s {
        "kitty" => Some(ImageMode::Kitty),
        "sixel" => Some(ImageMode::Sixel),
        "iterm2" => Some(ImageMode::ITerm2),
        "halfblock" => Some(ImageMode::Halfblock),
        _ => None,
    }
}

fn parse_theme(s: &str) -> Option<ThemeMode> {
    match s {
        "auto" => Some(ThemeMode::Auto),
        "light" => Some(ThemeMode::Light),
        "dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_flag_tokens_extracts_known_flags() {
        let args = vec![
            "markless".to_string(),
            "--watch".to_string(),
            "--toc".to_string(),
            "--no-images".to_string(),
            "--theme".to_string(),
            "dark".to_string(),
            "--render-debug-log=render.log".to_string(),
            "--force-half-cell".to_string(),
            "README.md".to_string(),
        ];
        let flags = parse_flag_tokens(&args);
        assert!(flags.watch);
        assert!(flags.toc);
        assert!(flags.no_images);
        assert_eq!(flags.theme, Some(ThemeMode::Dark));
        assert_eq!(flags.render_debug_log, Some(PathBuf::from("render.log")));
        assert!(flags.force_half_cell);
    }

    #[test]
    fn test_parse_flag_tokens_image_mode_kitty() {
        let args = vec!["--image-mode".to_string(), "kitty".to_string()];
        let flags = parse_flag_tokens(&args);
        assert_eq!(flags.image_mode, Some(ImageMode::Kitty));
    }

    #[test]
    fn test_parse_flag_tokens_image_mode_sixel() {
        let args = vec!["--image-mode=sixel".to_string()];
        let flags = parse_flag_tokens(&args);
        assert_eq!(flags.image_mode, Some(ImageMode::Sixel));
    }

    #[test]
    fn test_parse_flag_tokens_image_mode_iterm2() {
        let args = vec!["--image-mode".to_string(), "iterm2".to_string()];
        let flags = parse_flag_tokens(&args);
        assert_eq!(flags.image_mode, Some(ImageMode::ITerm2));
    }

    #[test]
    fn test_parse_flag_tokens_image_mode_halfblock() {
        let args = vec!["--image-mode=halfblock".to_string()];
        let flags = parse_flag_tokens(&args);
        assert_eq!(flags.image_mode, Some(ImageMode::Halfblock));
    }

    #[test]
    fn test_parse_flag_tokens_image_mode_invalid_ignored() {
        let args = vec!["--image-mode".to_string(), "invalid".to_string()];
        let flags = parse_flag_tokens(&args);
        assert_eq!(flags.image_mode, None);
    }

    #[test]
    fn test_parse_flag_tokens_force_half_cell_sets_image_mode() {
        let args = vec!["--force-half-cell".to_string()];
        let flags = parse_flag_tokens(&args);
        assert_eq!(flags.image_mode, Some(ImageMode::Halfblock));
    }

    #[test]
    fn test_config_union_image_mode_cli_overrides_file() {
        let file = ConfigFlags {
            image_mode: Some(ImageMode::Kitty),
            ..ConfigFlags::default()
        };
        let cli = ConfigFlags {
            image_mode: Some(ImageMode::Sixel),
            ..ConfigFlags::default()
        };
        let merged = file.union(&cli);
        assert_eq!(merged.image_mode, Some(ImageMode::Sixel));
    }

    #[test]
    fn test_config_union_image_mode_file_preserved_when_cli_none() {
        let file = ConfigFlags {
            image_mode: Some(ImageMode::ITerm2),
            ..ConfigFlags::default()
        };
        let cli = ConfigFlags::default();
        let merged = file.union(&cli);
        assert_eq!(merged.image_mode, Some(ImageMode::ITerm2));
    }

    #[test]
    fn test_save_load_image_mode_kitty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".marklessrc");
        let flags = ConfigFlags {
            image_mode: Some(ImageMode::Kitty),
            ..ConfigFlags::default()
        };
        save_config_flags(&path, &flags).unwrap();
        let loaded = load_config_flags(&path).unwrap();
        assert_eq!(loaded.image_mode, Some(ImageMode::Kitty));
    }

    #[test]
    fn test_save_load_image_mode_sixel() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".marklessrc");
        let flags = ConfigFlags {
            image_mode: Some(ImageMode::Sixel),
            ..ConfigFlags::default()
        };
        save_config_flags(&path, &flags).unwrap();
        let loaded = load_config_flags(&path).unwrap();
        assert_eq!(loaded.image_mode, Some(ImageMode::Sixel));
    }

    #[test]
    fn test_save_load_image_mode_iterm2() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".marklessrc");
        let flags = ConfigFlags {
            image_mode: Some(ImageMode::ITerm2),
            ..ConfigFlags::default()
        };
        save_config_flags(&path, &flags).unwrap();
        let loaded = load_config_flags(&path).unwrap();
        assert_eq!(loaded.image_mode, Some(ImageMode::ITerm2));
    }

    #[test]
    fn test_save_load_image_mode_halfblock() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".marklessrc");
        let flags = ConfigFlags {
            image_mode: Some(ImageMode::Halfblock),
            ..ConfigFlags::default()
        };
        save_config_flags(&path, &flags).unwrap();
        let loaded = load_config_flags(&path).unwrap();
        assert_eq!(loaded.image_mode, Some(ImageMode::Halfblock));
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
        let path = dir.path().join(".marklessrc");
        let flags = ConfigFlags {
            watch: true,
            toc: true,
            no_images: true,
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
        assert_eq!(loaded.no_images, true);
        assert_eq!(loaded.perf, true);
        assert_eq!(loaded.force_half_cell, true);
        assert_eq!(loaded.theme, Some(ThemeMode::Dark));
        assert_eq!(loaded.render_debug_log, Some(PathBuf::from("render.log")));

        clear_config_flags(&path).unwrap();
        assert!(!path.exists());
    }
}
