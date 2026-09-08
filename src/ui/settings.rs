use crate::app::Action;
use crate::config::{
    LATENCY_MS_RANGE, LONG_PRESS_MS_RANGE, MAX_LAYERS_RANGE, MAX_LOOP_SECS_RANGE, PREROLL_MS_RANGE,
    VOLUME_PCT_RANGE,
};
use crate::settings::SettingsState;

/// Reserved below the scrolling list so Start is always reachable.
const START_ROW_HEIGHT: f32 = 40.0;

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
            });

            // The list outgrew the window once the engine settings
            // joined it, so it scrolls - with Start kept outside, where
            // it can't end up below the fold.
            let can_start = egui::ScrollArea::vertical()
                .max_height((ui.available_height() - START_ROW_HEIGHT).max(120.0))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| controls(ui, settings)).inner
                })
                .inner;

            if !can_start {
                return;
            }

            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                if ui
                    .add_sized([140.0, 26.0], egui::Button::new("Start"))
                    .clicked()
                {
                    action = Some(Action::Start(settings.config.clone()));
                }
            });
        });

    action
}

/// The pickers and sliders. Returns whether there's a startable choice:
/// a device that reports no usable rate or no inputs can't be opened, and
/// says so instead of offering Start.
fn controls(ui: &mut egui::Ui, settings: &mut SettingsState) -> bool {
    if settings.devices.is_empty() {
        ui.colored_label(egui::Color32::RED, "No ASIO devices found.");
        return false;
    }

    let previous_device = settings.config.device_name.clone();
    egui::ComboBox::from_label("ASIO device")
        .selected_text(settings.config.device_name.clone())
        .show_ui(ui, |ui| {
            for name in &settings.devices {
                ui.selectable_value(&mut settings.config.device_name, name.clone(), name);
            }
        });
    if settings.config.device_name != previous_device {
        settings.refresh_for_selected_device();
    }

    if settings.sample_rates.is_empty() {
        ui.colored_label(
            egui::Color32::RED,
            "This device reports no supported sample rate.",
        );
        return false;
    }

    egui::ComboBox::from_label("Sample rate")
        .selected_text(format!("{} Hz", settings.config.sample_rate))
        .show_ui(ui, |ui| {
            for &rate in &settings.sample_rates {
                ui.selectable_value(&mut settings.config.sample_rate, rate, format!("{rate} Hz"));
            }
        });

    if settings.input_channels == 0 {
        ui.colored_label(egui::Color32::RED, "This device reports no input channels.");
        return false;
    }

    egui::ComboBox::from_label("Input channel")
        .selected_text(format!("Input {}", settings.config.input_channel + 1))
        .show_ui(ui, |ui| {
            for channel in 0..settings.input_channels {
                ui.selectable_value(
                    &mut settings.config.input_channel,
                    channel,
                    format!("Input {}", channel + 1),
                );
            }
        });

    ui.add_space(8.0);
    ui.separator();

    ui.add(
        egui::Slider::new(&mut settings.config.volume_pct, VOLUME_PCT_RANGE)
            .text("Loop volume")
            .suffix("%"),
    );
    ui.add(
        egui::Slider::new(&mut settings.config.preroll_ms, PREROLL_MS_RANGE)
            .step_by(500.0)
            .text("Record delay")
            // Shown in seconds, and as "off" rather than "0.0 s" so the
            // choice reads as deliberate.
            .custom_formatter(|ms, _| {
                if ms < 1.0 {
                    "off".to_owned()
                } else {
                    format!("{:.1} s", ms / 1000.0)
                }
            }),
    );
    ui.add(
        egui::Slider::new(&mut settings.config.long_press_ms, LONG_PRESS_MS_RANGE)
            .step_by(250.0)
            .text("Hold to clear")
            .custom_formatter(|ms, _| format!("{:.2} s", ms / 1000.0)),
    );

    ui.add_space(8.0);
    ui.separator();

    ui.add(
        egui::Slider::new(&mut settings.config.latency_ms, LATENCY_MS_RANGE)
            .text("Latency")
            .suffix(" ms"),
    );
    ui.add(
        egui::Slider::new(&mut settings.config.max_loop_secs, MAX_LOOP_SECS_RANGE)
            .step_by(10.0)
            .text("Max loop length")
            .suffix(" s"),
    );
    ui.add(egui::Slider::new(&mut settings.config.max_layers, MAX_LAYERS_RANGE).text("Max layers"));
    // Every layer is pre-allocated at the full loop length, so these two
    // settings have a price worth seeing before it's paid.
    ui.label(format!(
        "Reserves {} MB of memory",
        settings.config.loop_memory_bytes() / (1024 * 1024)
    ));

    true
}
