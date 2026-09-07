use crate::audio::engine;
use crate::config::AppConfig;

/// State behind the settings screen: the device/rate/channel lists to
/// choose from and what's currently selected. Plain data - the rendering
/// of it lives in `ui/settings.rs`.
pub struct SettingsState {
    pub devices: Vec<String>,
    pub selected_device: usize,
    pub sample_rates: Vec<u32>,
    pub selected_rate: usize,
    pub input_channels: u16,
    pub selected_input_channel: usize,
    /// Loop playback gain, 0-200% of unity - see `AppConfig::volume_pct`.
    pub volume_pct: u32,
    pub error: Option<String>,
}

impl SettingsState {
    /// Pre-selects whatever was last saved (or is currently running), so
    /// reopening settings doesn't reset your choices back to the top of
    /// each list.
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

        let volume_pct = saved.as_ref().map(|cfg| cfg.volume_pct).unwrap_or(100);

        Self {
            devices,
            selected_device,
            sample_rates,
            selected_rate,
            input_channels,
            selected_input_channel,
            volume_pct,
            error,
        }
    }

    /// Rates and channel counts are per-device, so switching device
    /// re-queries them and drops selections that no longer apply.
    pub fn refresh_for_selected_device(&mut self) {
        if let Some(name) = self.devices.get(self.selected_device) {
            (self.sample_rates, self.input_channels) = engine::rates_and_channels(name);
            self.selected_rate = 0;
            self.selected_input_channel = 0;
        }
    }
}
