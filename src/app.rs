use crate::config::AppConfig;
use crate::looper::LooperState;
use crate::settings::SettingsState;

/// Which screen the app is on, and the state that screen needs. Plain
/// data: no `egui::` types here or in either of the states it holds, so
/// the app model isn't tied to the GUI library drawing it - `main.rs` is
/// the only place that knows eframe exists.
pub enum Screen {
    Settings(SettingsState),
    Looper(LooperState),
}

/// What a rendered frame asks the app to do next. Renderers only report
/// intent (see `ui/`); acting on it - opening the device, persisting the
/// choice, switching screens - happens here.
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
    /// Starts straight into the looper if there's a saved config that
    /// still opens; otherwise shows Settings, carrying the failure so it
    /// can say why (e.g. the interface was unplugged).
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

    /// Opens the device and switches to the looper, persisting the choice
    /// only once it's known to actually work. On failure the settings
    /// screen stays put and shows the error instead.
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
