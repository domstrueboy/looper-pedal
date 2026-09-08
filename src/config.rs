use std::ops::RangeInclusive;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Defaults, and the range each setting is allowed to take. The settings
/// screen builds its sliders from these, and `clamped` holds a
/// hand-edited file to them, so the two can't drift apart.
pub const VOLUME_PCT_RANGE: RangeInclusive<u32> = 0..=200;
pub const DEFAULT_VOLUME_PCT: u32 = 100;

/// Wait before recording starts. Long enough by default to put a guitar
/// back on and be ready; zero turns it off.
pub const PREROLL_MS_RANGE: RangeInclusive<u32> = 0..=5000;
pub const DEFAULT_PREROLL_MS: u32 = 5000;

/// Headroom between the input and output callbacks, to absorb timing
/// jitter. The setting to raise when the log complains about underruns -
/// at the cost of monitoring being that much less immediate.
pub const LATENCY_MS_RANGE: RangeInclusive<u32> = 2..=50;
pub const DEFAULT_LATENCY_MS: u32 = 8;

/// Longest loop that can be recorded. Every layer is pre-allocated at
/// this length, so together with `max_layers` it decides how much memory
/// the app takes: seconds x layers x sample rate x 4 bytes.
pub const MAX_LOOP_SECS_RANGE: RangeInclusive<u32> = 10..=120;
pub const DEFAULT_MAX_LOOP_SECS: u32 = 60;

/// How many layers can be stacked, the first recording included.
pub const MAX_LAYERS_RANGE: RangeInclusive<u32> = 1..=8;
pub const DEFAULT_MAX_LAYERS: u32 = 4;

/// How long the control has to be held to clear the loop.
pub const LONG_PRESS_MS_RANGE: RangeInclusive<u32> = 500..=4000;
pub const DEFAULT_LONG_PRESS_MS: u32 = 2000;

const CONFIG_DIR: &str = "looper-pedal";
const CONFIG_FILE: &str = "config.toml";
/// The old config: a hand-rolled `key=value` file next to the
/// executable. Read once, to migrate - see `load`.
const LEGACY_FILE: &str = "looper-pedal.cfg";
const LOOP_DIR: &str = "loop";

/// Persisted device/rate/input-channel/volume/pre-roll choice, which
/// doubles as the bundle of settings the looper is started with.
///
/// Settings added after the first release carry defaults, so a config
/// written by an older build still loads rather than throwing the user
/// back to the settings screen. The two that identify the device don't:
/// without them there's nothing to open, so such a file counts as no
/// config at all.
#[derive(Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub device_name: String,
    pub sample_rate: u32,
    /// 0-indexed input channel to capture/record/loop.
    #[serde(default)]
    pub input_channel: u16,
    /// Loop playback gain as a percentage of unity (100 = unchanged).
    /// Applied to the loop only, never the live passthrough.
    #[serde(default = "default_volume_pct")]
    pub volume_pct: u32,
    /// How long to wait between pressing record and actually capturing,
    /// so there's time to get ready. 0 = start immediately.
    #[serde(default = "default_preroll_ms")]
    pub preroll_ms: u32,
    /// See `LATENCY_MS_RANGE`.
    #[serde(default = "default_latency_ms")]
    pub latency_ms: u32,
    /// See `MAX_LOOP_SECS_RANGE`.
    #[serde(default = "default_max_loop_secs")]
    pub max_loop_secs: u32,
    /// See `MAX_LAYERS_RANGE`.
    #[serde(default = "default_max_layers")]
    pub max_layers: u32,
    /// See `LONG_PRESS_MS_RANGE`.
    #[serde(default = "default_long_press_ms")]
    pub long_press_ms: u32,
}

fn default_volume_pct() -> u32 {
    DEFAULT_VOLUME_PCT
}

fn default_preroll_ms() -> u32 {
    DEFAULT_PREROLL_MS
}

fn default_latency_ms() -> u32 {
    DEFAULT_LATENCY_MS
}

fn default_max_loop_secs() -> u32 {
    DEFAULT_MAX_LOOP_SECS
}

fn default_max_layers() -> u32 {
    DEFAULT_MAX_LAYERS
}

fn default_long_press_ms() -> u32 {
    DEFAULT_LONG_PRESS_MS
}

impl AppConfig {
    /// Reads the config, migrating one from next to the executable if
    /// that's all there is. `None` means there's nothing usable and the
    /// caller should show the settings picker.
    pub fn load() -> Option<Self> {
        let current = std::fs::read_to_string(config_path())
            .ok()
            .and_then(|text| Self::from_toml(&text));
        if current.is_some() {
            return current;
        }

        let migrated = std::fs::read_to_string(legacy_path())
            .ok()
            .and_then(|text| Self::from_legacy(&text))?;
        // Write it where it belongs now, so this happens once. The old
        // file is left in place rather than deleted - it does no harm,
        // and an older build can still read it.
        let _ = migrated.save();
        Some(migrated)
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path();
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|e| format!("creating {}: {e}", directory.display()))?;
        }
        std::fs::write(&path, self.to_toml()?)
            .map_err(|e| format!("writing {}: {e}", path.display()))
    }

    fn from_toml(text: &str) -> Option<Self> {
        toml::from_str::<Self>(text).ok().map(Self::clamped)
    }

    /// Holds every setting to its supported range, so a hand-edited file
    /// can't ask for something absurd - a thousand layers of a
    /// two-minute loop would try to allocate tens of gigabytes before the
    /// window even opened.
    fn clamped(mut self) -> Self {
        self.volume_pct = clamp_to(self.volume_pct, VOLUME_PCT_RANGE);
        self.preroll_ms = clamp_to(self.preroll_ms, PREROLL_MS_RANGE);
        self.latency_ms = clamp_to(self.latency_ms, LATENCY_MS_RANGE);
        self.max_loop_secs = clamp_to(self.max_loop_secs, MAX_LOOP_SECS_RANGE);
        self.max_layers = clamp_to(self.max_layers, MAX_LAYERS_RANGE);
        self.long_press_ms = clamp_to(self.long_press_ms, LONG_PRESS_MS_RANGE);
        self
    }

    /// Memory the loop layers will take at `sample_rate`, in bytes: every
    /// layer is pre-allocated at the full loop length, so this is the
    /// price of the two settings that decide it.
    pub fn loop_memory_bytes(&self) -> u64 {
        u64::from(self.max_loop_secs)
            * u64::from(self.max_layers)
            * u64::from(self.sample_rate)
            * size_of::<i32>() as u64
    }

    fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| format!("serializing config: {e}"))
    }

    /// The old `key=value` format, read only to migrate it. Can go once
    /// no such file is likely to be left anywhere.
    fn from_legacy(text: &str) -> Option<Self> {
        let mut device_name = None;
        let mut sample_rate = None;
        let mut input_channel = None;
        let mut volume_pct = None;
        let mut preroll_ms = None;

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "device_name" => device_name = Some(value.to_string()),
                "sample_rate" => sample_rate = value.parse().ok(),
                "input_channel" => input_channel = value.parse().ok(),
                "volume_pct" => volume_pct = value.parse().ok(),
                "preroll_ms" => preroll_ms = value.parse().ok(),
                _ => {}
            }
        }

        Some(
            Self {
                device_name: device_name?,
                sample_rate: sample_rate?,
                input_channel: input_channel.unwrap_or(0),
                volume_pct: volume_pct.unwrap_or(DEFAULT_VOLUME_PCT),
                preroll_ms: preroll_ms.unwrap_or(DEFAULT_PREROLL_MS),
                // Nothing after this existed in that format.
                latency_ms: DEFAULT_LATENCY_MS,
                max_loop_secs: DEFAULT_MAX_LOOP_SECS,
                max_layers: DEFAULT_MAX_LAYERS,
                long_press_ms: DEFAULT_LONG_PRESS_MS,
            }
            .clamped(),
        )
    }
}

impl Default for AppConfig {
    /// Every setting at its default, with no device chosen yet: what the
    /// settings screen starts from when there's nothing saved.
    ///
    /// Deliberately not what `serde` falls back to - `device_name` and
    /// `sample_rate` stay required in the file, because a config without
    /// them names nothing to open. The values come from the same consts
    /// the per-field defaults do, so there's still one declaration each.
    fn default() -> Self {
        Self {
            device_name: String::new(),
            sample_rate: 0,
            input_channel: 0,
            volume_pct: DEFAULT_VOLUME_PCT,
            preroll_ms: DEFAULT_PREROLL_MS,
            latency_ms: DEFAULT_LATENCY_MS,
            max_loop_secs: DEFAULT_MAX_LOOP_SECS,
            max_layers: DEFAULT_MAX_LAYERS,
            long_press_ms: DEFAULT_LONG_PRESS_MS,
        }
    }
}

/// The per-user config directory - `%APPDATA%\looper-pedal` on Windows,
/// `~/.config/looper-pedal` on Linux, `~/Library/Application
/// Support/looper-pedal` on macOS. Next to the executable stopped being
/// the right place because it isn't writable once the app is installed
/// somewhere like Program Files; that's only the fallback for having no
/// home directory at all.
fn config_path() -> PathBuf {
    app_dir().join(CONFIG_FILE)
}

/// The recorded loop, one WAV per layer, kept beside the config.
pub fn loop_dir() -> PathBuf {
    app_dir().join(LOOP_DIR)
}

fn app_dir() -> PathBuf {
    match directories::BaseDirs::new() {
        Some(dirs) => dirs.config_dir().join(CONFIG_DIR),
        None => exe_dir(),
    }
}

fn legacy_path() -> PathBuf {
    exe_dir().join(LEGACY_FILE)
}

fn clamp_to(value: u32, range: RangeInclusive<u32>) -> u32 {
    value.clamp(*range.start(), *range.end())
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
