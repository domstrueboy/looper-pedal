use crate::audio::engine;
use crate::config::*;

/// State behind the settings screen: what there is to choose from and
/// what's selected. Rendered by `ui/settings.rs`.
pub struct SettingsState {
    pub devices: Vec<String>,
    pub selected_device: usize,
    pub sample_rates: Vec<u32>,
    pub selected_rate: usize,
    pub input_channels: u16,
    pub selected_input_channel: usize,
    /// Loop playback gain, 0-200% of unity - see `AppConfig::volume_pct`.
    pub volume_pct: u32,
    /// The value settings, in the same units and ranges `AppConfig`
    /// documents - the sliders are built from the ranges declared there.
    pub preroll_ms: u32,
    pub latency_ms: u32,
    pub max_loop_secs: u32,
    pub max_layers: u32,
    pub long_press_ms: u32,
    pub error: Option<String>,
}

impl SettingsState {
    /// Pre-selects whatever is saved or currently running, so reopening
    /// settings doesn't reset every list to the top.
    pub fn new(error: Option<String>) -> Self {
        let devices = engine::available_asio_devices().unwrap_or_default();
        let saved = AppConfig::load();

        let selected_device = saved
            .as_ref()
            .and_then(|cfg| devices.iter().position(|d| *d == cfg.device_name))
            .unwrap_or(0);

        let (sample_rates, input_channels) = match devices.get(selected_device) {
            Some(name) => engine::rates_and_channels(name),
            None => (Vec::new(), 0),
        };

        let selected_rate = saved
            .as_ref()
            .and_then(|cfg| sample_rates.iter().position(|&r| r == cfg.sample_rate))
            .unwrap_or(0);

        let selected_input_channel = saved
            .as_ref()
            .map(|cfg| cfg.input_channel as usize)
            .filter(|&ch| ch < input_channels as usize)
            .unwrap_or(0);

        let saved_value =
            |pick: fn(&AppConfig) -> u32, default: u32| saved.as_ref().map(pick).unwrap_or(default);
        let volume_pct = saved_value(|c| c.volume_pct, DEFAULT_VOLUME_PCT);
        let preroll_ms = saved_value(|c| c.preroll_ms, DEFAULT_PREROLL_MS);
        let latency_ms = saved_value(|c| c.latency_ms, DEFAULT_LATENCY_MS);
        let max_loop_secs = saved_value(|c| c.max_loop_secs, DEFAULT_MAX_LOOP_SECS);
        let max_layers = saved_value(|c| c.max_layers, DEFAULT_MAX_LAYERS);
        let long_press_ms = saved_value(|c| c.long_press_ms, DEFAULT_LONG_PRESS_MS);

        Self {
            devices,
            selected_device,
            sample_rates,
            selected_rate,
            input_channels,
            selected_input_channel,
            volume_pct,
            preroll_ms,
            latency_ms,
            max_loop_secs,
            max_layers,
            long_press_ms,
            error,
        }
    }

    /// The current choice as a config, ready to start the looper with and
    /// to persist.
    pub fn to_config(&self) -> AppConfig {
        AppConfig {
            device_name: self
                .devices
                .get(self.selected_device)
                .cloned()
                .unwrap_or_default(),
            sample_rate: self
                .sample_rates
                .get(self.selected_rate)
                .copied()
                .unwrap_or_default(),
            input_channel: self.selected_input_channel as u16,
            volume_pct: self.volume_pct,
            preroll_ms: self.preroll_ms,
            latency_ms: self.latency_ms,
            max_loop_secs: self.max_loop_secs,
            max_layers: self.max_layers,
            long_press_ms: self.long_press_ms,
        }
    }

    /// What the loop length and layer count will cost in memory, since
    /// every layer is pre-allocated in full.
    pub fn loop_memory_bytes(&self) -> u64 {
        self.to_config().loop_memory_bytes()
    }

    /// Rates and channels are per-device, so a device change re-queries
    /// them and drops selections that no longer apply.
    pub fn refresh_for_selected_device(&mut self) {
        if let Some(name) = self.devices.get(self.selected_device) {
            (self.sample_rates, self.input_channels) = engine::rates_and_channels(name);
            self.selected_rate = 0;
            self.selected_input_channel = 0;
        }
    }
}
