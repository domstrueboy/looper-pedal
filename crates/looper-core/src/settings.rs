use looper_hal::Backends;

use crate::audio::engine;
use crate::config::AppConfig;

/// State behind the settings screen: what there is to choose from, and
/// the choice itself. Rendered by the UI crate's settings screen.
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
    /// The saved choice, with nothing looked up yet.
    ///
    /// Asking the backends is `refresh`, deliberately not done here: the
    /// caller has to be able to stop a running looper before anything
    /// enumerates, and it can only do that once this screen exists to
    /// put in its place.
    pub fn new(error: Option<String>) -> Self {
        Self {
            devices: Vec::new(),
            sample_rates: Vec::new(),
            input_channels: 0,
            config: AppConfig::load().unwrap_or_default(),
            error,
        }
    }

    /// Asks what there is, and pre-selects whatever is saved, so
    /// reopening settings doesn't reset every list to the top.
    pub fn refresh(&mut self, backends: &Backends) {
        self.devices = backends
            .devices()
            .into_iter()
            // A device that can only play is no use: the looper has to
            // record from whatever it opens.
            .filter(|device| device.direction.can_capture())
            .map(|device| device.id.name)
            .collect();

        // A saved device that isn't plugged in now falls back to the
        // first one there is, rather than leaving a name nothing matches.
        if !self.devices.contains(&self.config.device_name) {
            self.config.device_name = self.devices.first().cloned().unwrap_or_default();
        }
        self.refresh_for_selected_device(backends);
    }

    /// Rates and channels are per-device, so a device change re-queries
    /// them and drops any selection the new device doesn't offer. A
    /// selection it does offer is kept - switching devices shouldn't
    /// silently move a rate that both of them support.
    pub fn refresh_for_selected_device(&mut self, backends: &Backends) {
        let caps = engine::find_device(backends, &self.config.device_name)
            .ok()
            .and_then(|device| backends.get(device.backend)?.caps(&device).ok());

        (self.sample_rates, self.input_channels) = match caps {
            // Falls back to empty rather than surfacing the error: a
            // device that answers neither can't be started, and the
            // screen says so from the empty lists themselves.
            Some(caps) => (caps.sample_rates, caps.input_channels),
            None => (Vec::new(), 0),
        };

        if !self.sample_rates.contains(&self.config.sample_rate) {
            self.config.sample_rate = self.sample_rates.first().copied().unwrap_or_default();
        }
        if self.config.input_channel >= self.input_channels {
            self.config.input_channel = 0;
        }
    }
}
