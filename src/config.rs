use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Built-in palettes, keyed by the canonical theme name that
/// `~/.config/apply-theme.sh` writes into our config.
///
/// Five colours per theme, in depth order: blue, green, yellow, magenta, cyan.
/// Red is deliberately absent — in this setup it carries "failing CI / changes
/// requested" everywhere else on screen, and a container outline must not read
/// as an error.
const PALETTES: &[(&str, &[&str])] = &[
    // folke Tokyo Night "Night", matching ~/.config/gh-dash/config.yml
    ("tokyo-night", &["#7aa2f7", "#9ece6a", "#e0af68", "#bb9af7", "#7dcfff"]),
    ("catppuccin-mocha", &["#89b4fa", "#a6e3a1", "#f9e2af", "#cba6f7", "#89dceb"]),
    ("rose-pine", &["#31748f", "#9ccfd8", "#f6c177", "#c4a7e7", "#ebbcba"]),
];

const FALLBACK_THEME: &str = "tokyo-night";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Canonical theme name. Written by `apply-theme.sh`; selects a built-in
    /// palette. Ignored when `palette` is set.
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Explicit colours, `#rrggbb` or `#aarrggbb`. Overrides the theme palette.
    #[serde(default)]
    pub palette: Option<Vec<String>>,

    #[serde(default = "default_flash_ms")]
    pub flash_ms: u64,
    /// Total width of the fading band, in points. The band starts opaque at
    /// the container's outer edge and reaches fully transparent this far
    /// inward, so it overlaps the member windows by roughly this much.
    #[serde(default = "default_band_width")]
    pub band_width: f64,
    /// Extra outward growth beyond half the inner gap, so the band's outer
    /// edge clears the member windows.
    #[serde(default = "default_outset")]
    pub outset: f64,
    #[serde(default = "default_corner_radius")]
    pub corner_radius: f64,
    /// Extra inset per nesting level, so concentric outlines stay separated.
    #[serde(default = "default_level_inset")]
    pub level_inset: f64,
    /// Alpha multiplier for containers that do not hold the selection.
    #[serde(default = "default_dim_factor")]
    pub dim_factor: f64,

    /// Overrides the gaps read from rift's own config.
    #[serde(default)]
    pub gaps: Option<GapsOverride>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapsOverride {
    pub inner_h: f64,
    pub inner_v: f64,
}

fn default_theme() -> String { FALLBACK_THEME.to_string() }
fn default_flash_ms() -> u64 { 1500 }
fn default_band_width() -> f64 { 36.0 }
fn default_outset() -> f64 { 4.0 }
// macOS 26 rounds window corners considerably more than earlier releases.
// JankyBorders reads the real per-window value via SLSWindowIteratorGetCornerRadii
// and falls back to 9; reading it would mean vendoring the window-iterator API,
// so this is a tunable default instead.
fn default_corner_radius() -> f64 { 22.0 }
fn default_level_inset() -> f64 { 3.0 }
fn default_dim_factor() -> f64 { 0.45 }

impl Config {
    pub fn load() -> Result<Config> {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => Config::from_str(&s).with_context(|| format!("parsing {}", path.display())),
            // No config file is the normal case, not an error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::from_str(""),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn from_str(s: &str) -> Result<Config> {
        let cfg: Config = toml::from_str(s)?;
        // Fail loudly on a malformed explicit palette rather than silently
        // falling back to the theme — a typo in a colour should be visible.
        if let Some(p) = &cfg.palette {
            if p.is_empty() {
                bail!("palette is set but empty; remove the key to use the theme palette");
            }
            for c in p {
                parse_color(c)?;
            }
        }
        Ok(cfg)
    }

    /// Resolved colours as `0xAARRGGBB`.
    pub fn colors(&self) -> Vec<u32> {
        if let Some(p) = &self.palette {
            // Already validated in `from_str`.
            return p.iter().filter_map(|c| parse_color(c).ok()).collect();
        }
        let hexes = PALETTES
            .iter()
            .find(|(name, _)| *name == self.theme)
            .or_else(|| PALETTES.iter().find(|(name, _)| *name == FALLBACK_THEME))
            .map(|(_, cs)| *cs)
            .unwrap_or(&[]);
        hexes.iter().filter_map(|c| parse_color(c).ok()).collect()
    }

    /// Colour for a container at `depth` (1-based; root is depth 0 and undrawn).
    /// Wraps when nesting runs deeper than the palette.
    pub fn color_for_depth(&self, depth: usize) -> u32 {
        let cs = self.colors();
        debug_assert!(!cs.is_empty(), "colors() must never be empty");
        let i = depth.saturating_sub(1) % cs.len();
        cs[i]
    }
}

fn config_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("rift-container-highlighter").join("config.toml")
}

/// `#rrggbb` (opaque) or `#aarrggbb`, to `0xAARRGGBB`.
fn parse_color(s: &str) -> Result<u32> {
    let h = s.strip_prefix('#').unwrap_or(s);
    let v = u32::from_str_radix(h, 16)
        .with_context(|| format!("colour {s:?} is not hex; expected #rrggbb or #aarrggbb"))?;
    match h.len() {
        6 => Ok(0xff00_0000 | v),
        8 => Ok(v),
        _ => bail!("colour {s:?} must have 6 or 8 hex digits, got {}", h.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_when_no_file_exists() {
        let c = Config::from_str("").unwrap();
        assert_eq!(c.flash_ms, 1500);
        assert_eq!(c.theme, "tokyo-night");
        assert!(c.band_width > 0.0);
        assert!(c.palette.is_none());
    }

    #[test]
    fn tokyo_night_palette_matches_the_setup() {
        let c = Config::from_str("").unwrap();
        // #7aa2f7 is the blue used by gh-dash's `actor` slot.
        assert_eq!(c.colors()[0], 0xff7a_a2f7);
        assert_eq!(c.colors().len(), 5);
    }

    #[test]
    fn red_is_not_in_any_palette() {
        // Red means "failing" elsewhere in this setup; an outline must not.
        for (name, _) in PALETTES {
            let c = Config::from_str(&format!("theme = {name:?}")).unwrap();
            assert!(
                !c.colors().contains(&0xfff7_768e),
                "{name} palette must not include tokyo night red"
            );
        }
    }

    #[test]
    fn known_theme_selects_its_palette() {
        let a = Config::from_str(r##"theme = "catppuccin-mocha""##).unwrap();
        let b = Config::from_str(r##"theme = "tokyo-night""##).unwrap();
        assert!(!a.colors().is_empty());
        assert_ne!(a.colors(), b.colors());
    }

    #[test]
    fn unknown_theme_falls_back_without_error() {
        let c = Config::from_str(r##"theme = "not-a-theme""##).unwrap();
        assert_eq!(c.colors(), Config::from_str("").unwrap().colors());
    }

    #[test]
    fn explicit_palette_overrides_the_theme() {
        let c = Config::from_str(
            r##"
            theme = "tokyo-night"
            palette = ["#112233", "#445566"]
            "##,
        )
        .unwrap();
        assert_eq!(c.colors(), vec![0xff11_2233, 0xff44_5566]);
    }

    #[test]
    fn eight_digit_colors_keep_their_alpha() {
        let c = Config::from_str(r##"palette = ["#80112233"]"##).unwrap();
        assert_eq!(c.colors(), vec![0x8011_2233]);
    }

    #[test]
    fn a_malformed_color_is_an_error_not_a_silent_fallback() {
        assert!(Config::from_str(r##"palette = ["#xyz"]"##).is_err());
        assert!(Config::from_str(r##"palette = ["#12345"]"##).is_err());
        assert!(Config::from_str(r##"palette = []"##).is_err());
    }

    #[test]
    fn depth_wraps_around_the_palette() {
        let c = Config::from_str(r##"palette = ["#000001", "#000002"]"##).unwrap();
        assert_eq!(c.color_for_depth(1), 0xff00_0001);
        assert_eq!(c.color_for_depth(2), 0xff00_0002);
        assert_eq!(c.color_for_depth(3), 0xff00_0001);
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        // A typo'd key must not be silently ignored.
        assert!(Config::from_str("flsah_ms = 100").is_err());
        // stroke_width was replaced by band_width; the old key must not pass
        // silently.
        assert!(Config::from_str("stroke_width = 2.0").is_err());
    }
}
