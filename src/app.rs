use crate::config::AppConfig;
use crate::looper::LooperState;
use crate::settings::SettingsState;

/// Which screen is up, and that screen's state - plain data, no `egui::`
/// types.
pub enum Screen {
    Settings(SettingsState),
    Looper(LooperState),
}

/// What a rendered frame asks the app to do next: `ui/` reports intent,
/// `apply` acts on it.
pub enum Action {
    Start {
        device_name: String,
        sample_rate: u32,
        input_channel: u16,
        volume_pct: u32,
    },
    OpenSettings,
}

pub struct App {
    pub screen: Screen,
}

impl App {
    /// Straight into the looper if a saved config still opens, otherwise
    /// Settings, carrying the failure so it can say why.
    pub fn new() -> Self {
        let screen = match AppConfig::load() {
            Some(cfg) => match LooperState::start(
                &cfg.device_name,
                cfg.sample_rate,
                cfg.input_channel,
                cfg.volume_pct,
            ) {
                Ok(looper) => Screen::Looper(looper),
                Err(err) => Screen::Settings(SettingsState::new(Some(err))),
            },
            None => Screen::Settings(SettingsState::new(None)),
        };
        Self { screen }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Start {
                device_name,
                sample_rate,
                input_channel,
                volume_pct,
            } => self.start_looper(device_name, sample_rate, input_channel, volume_pct),
            Action::OpenSettings => self.screen = Screen::Settings(SettingsState::new(None)),
        }
    }

    /// Persists the choice only once the device is known to open; on
    /// failure Settings stays put and shows why.
    fn start_looper(
        &mut self,
        device_name: String,
        sample_rate: u32,
        input_channel: u16,
        volume_pct: u32,
    ) {
        match LooperState::start(&device_name, sample_rate, input_channel, volume_pct) {
            Ok(looper) => {
                let _ = AppConfig {
                    device_name,
                    sample_rate,
                    input_channel,
                    volume_pct,
                }
                .save();
                self.screen = Screen::Looper(looper);
            }
            Err(err) => {
                if let Screen::Settings(settings) = &mut self.screen {
                    settings.error = Some(err);
                }
            }
        }
    }
}
