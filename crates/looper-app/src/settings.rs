use crate::cpal_engine as engine;
use looper_core::config::AppConfig;

/// State behind the settings screen: what there is to choose from, and
/// the choice itself. Rendered by `ui/settings.rs`.
pub struct SettingsState {
    /// Per-device, so they're re-queried whenever the device changes.
    pub devices: Vec<String>,
    pub sample_rates: Vec<u32>,
    pub input_channels: u16,
    /// The choice, already in the shape it will be started and saved in.
    /// Held as an `AppConfig` rather than as a field per setting: a
    /// screen-shaped copy has to be mapped back and forth, and every
    /// setting added has to be added to both.
    pub config: AppConfig,
    pub error: Option<String>,
}

impl SettingsState {
    /// Pre-selects whatever is saved or currently running, so reopening
    /// settings doesn't reset every list to the top.
    pub fn new(error: Option<String>) -> Self {
        let devices = engine::available_asio_devices().unwrap_or_default();
        let config = AppConfig::load().unwrap_or_default();

        let mut settings = Self {
            devices,
            sample_rates: Vec::new(),
            input_channels: 0,
            config,
            error,
        };
        // A saved device that isn't plugged in now falls back to the
        // first one there is, rather than leaving a name nothing matches.
        if !settings.devices.contains(&settings.config.device_name) {
            settings.config.device_name = settings.devices.first().cloned().unwrap_or_default();
        }
        settings.refresh_for_selected_device();
        settings
    }

    /// Rates and channels are per-device, so a device change re-queries
    /// them and drops any selection the new device doesn't offer. A
    /// selection it does offer is kept - switching devices shouldn't
    /// silently move a rate that both of them support.
    pub fn refresh_for_selected_device(&mut self) {
        (self.sample_rates, self.input_channels) = if self.config.device_name.is_empty() {
            (Vec::new(), 0)
        } else {
            engine::rates_and_channels(&self.config.device_name)
        };

        if !self.sample_rates.contains(&self.config.sample_rate) {
            self.config.sample_rate = self.sample_rates.first().copied().unwrap_or_default();
        }
        if self.config.input_channel >= self.input_channels {
            self.config.input_channel = 0;
        }
    }
}
