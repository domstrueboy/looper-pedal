use crate::app::Action;
use crate::config::AppConfig;
use crate::settings::SettingsState;

pub fn render(ui: &mut egui::Ui, settings: &mut SettingsState) -> Option<Action> {
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
                ui.add(
                    egui::Slider::new(&mut settings.volume_pct, 0..=200)
                        .text("Loop volume")
                        .suffix("%"),
                );
                ui.add(
                    egui::Slider::new(&mut settings.preroll_ms, 0..=5000)
                        .step_by(500.0)
                        .text("Record delay")
                        // Shown in seconds, and as "off" rather than
                        // "0.0 s" so the default reads as a choice.
                        .custom_formatter(|ms, _| {
                            if ms < 1.0 {
                                "off".to_owned()
                            } else {
                                format!("{:.1} s", ms / 1000.0)
                            }
                        }),
                );

                ui.add_space(8.0);
                if ui.button("Start").clicked() {
                    action = Some(Action::Start(AppConfig {
                        device_name: settings.devices[settings.selected_device].clone(),
                        sample_rate: settings.sample_rates[settings.selected_rate],
                        input_channel: settings.selected_input_channel as u16,
                        volume_pct: settings.volume_pct,
                        preroll_ms: settings.preroll_ms,
                    }));
                }
            });
        });

    action
}
