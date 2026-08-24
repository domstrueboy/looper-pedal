use crate::audio::engine;
use crate::config::AppConfig;

pub struct SettingsState {
    pub devices: Vec<String>,
    pub selected_device: usize,
    pub sample_rates: Vec<u32>,
    pub selected_rate: usize,
    pub input_channels: u16,
    pub selected_input_channel: usize,
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

        Self {
            devices,
            selected_device,
            sample_rates,
            selected_rate,
            input_channels,
            selected_input_channel,
            error,
        }
    }

    fn refresh_for_selected_device(&mut self) {
        if let Some(name) = self.devices.get(self.selected_device) {
            (self.sample_rates, self.input_channels) = engine::rates_and_channels(name);
            self.selected_rate = 0;
            self.selected_input_channel = 0;
        }
    }
}

/// What the caller should do after a frame of the settings screen - the
/// caller owns actually starting the looper (and persisting the choice),
/// this module only renders and reports intent.
pub enum SettingsAction {
    Start {
        device_name: String,
        sample_rate: u32,
        input_channel: u16,
    },
}

pub fn render(ui: &mut egui::Ui, settings: &mut SettingsState) -> Option<SettingsAction> {
    let mut action = None;

    egui::Frame::default()
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label("Looper Pedal - Settings");
                ui.add_space(8.0);

                if let Some(err) = &settings.error {
                    ui.colored_label(egui::Color32::RED, err.as_str());
                    ui.add_space(8.0);
                }

                if settings.devices.is_empty() {
                    ui.colored_label(egui::Color32::RED, "No ASIO devices found.");
                    return;
                }

                let previous_device = settings.selected_device;
                egui::ComboBox::from_label("ASIO device")
                    .selected_text(settings.devices[settings.selected_device].clone())
                    .show_ui(ui, |ui| {
                        for (i, name) in settings.devices.iter().enumerate() {
                            ui.selectable_value(&mut settings.selected_device, i, name);
                        }
                    });
                if settings.selected_device != previous_device {
                    settings.refresh_for_selected_device();
                }

                if settings.sample_rates.is_empty() {
                    ui.colored_label(
                        egui::Color32::RED,
                        "This device reports no supported sample rate.",
                    );
                    return;
                }

                egui::ComboBox::from_label("Sample rate")
                    .selected_text(format!(
                        "{} Hz",
                        settings.sample_rates[settings.selected_rate]
                    ))
                    .show_ui(ui, |ui| {
                        for (i, rate) in settings.sample_rates.iter().enumerate() {
                            ui.selectable_value(
                                &mut settings.selected_rate,
                                i,
                                format!("{rate} Hz"),
                            );
                        }
                    });

                if settings.input_channels == 0 {
                    ui.colored_label(
                        egui::Color32::RED,
                        "This device reports no input channels.",
                    );
                    return;
                }

                egui::ComboBox::from_label("Input channel")
                    .selected_text(format!("Input {}", settings.selected_input_channel + 1))
                    .show_ui(ui, |ui| {
                        for i in 0..settings.input_channels as usize {
                            ui.selectable_value(
                                &mut settings.selected_input_channel,
                                i,
                                format!("Input {}", i + 1),
                            );
                        }
                    });

                ui.add_space(8.0);
                if ui.button("Start").clicked() {
                    action = Some(SettingsAction::Start {
                        device_name: settings.devices[settings.selected_device].clone(),
                        sample_rate: settings.sample_rates[settings.selected_rate],
                        input_channel: settings.selected_input_channel as u16,
                    });
                }
            });
        });

    action
}
