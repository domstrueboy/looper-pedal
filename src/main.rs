mod audio;
mod config;
mod input;
mod ui;

use std::sync::Arc;
use std::time::Instant;

use audio::engine;
use audio::shared_control::SharedControl;
use audio::state_machine::LoopStateMachine;
use config::AppConfig;
use input::{InputEvent, InputHandler};

struct LooperState {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    input_handler: InputHandler,
    sample_rate: u32,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

struct SettingsState {
    devices: Vec<String>,
    selected_device: usize,
    sample_rates: Vec<u32>,
    selected_rate: usize,
    input_channels: u16,
    selected_input_channel: usize,
    error: Option<String>,
}

impl SettingsState {
    /// Pre-selects whatever was last saved (or is currently running), so
    /// reopening settings doesn't reset your choices back to the top of
    /// each list.
    fn new(error: Option<String>) -> Self {
        let devices = engine::available_asio_devices().unwrap_or_default();
        let saved = AppConfig::load();

        let selected_device = saved
            .as_ref()
            .and_then(|cfg| devices.iter().position(|d| *d == cfg.device_name))
            .unwrap_or(0);

        let (sample_rates, input_channels) = match devices.get(selected_device) {
            Some(name) => (
                engine::supported_sample_rates(name).unwrap_or_default(),
                engine::input_channel_count(name).unwrap_or(0),
            ),
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
            self.sample_rates = engine::supported_sample_rates(name).unwrap_or_default();
            self.selected_rate = 0;
            self.input_channels = engine::input_channel_count(name).unwrap_or(0);
            self.selected_input_channel = 0;
        }
    }
}

enum Screen {
    Settings(SettingsState),
    Looper(LooperState),
}

struct LooperPedalApp {
    screen: Screen,
}

impl LooperPedalApp {
    fn try_start_looper(
        device_name: &str,
        sample_rate: u32,
        input_channel: u16,
    ) -> Result<LooperState, String> {
        let control = Arc::new(SharedControl::new());
        let (_input_stream, _output_stream, sample_rate) = engine::build_looper_streams(
            Arc::clone(&control),
            device_name,
            sample_rate,
            input_channel,
        )?;
        Ok(LooperState {
            control,
            state_machine: LoopStateMachine::new(),
            input_handler: InputHandler::new(),
            sample_rate,
            _input_stream,
            _output_stream,
        })
    }

    fn initial_screen() -> Screen {
        match AppConfig::load() {
            Some(cfg) => {
                match Self::try_start_looper(&cfg.device_name, cfg.sample_rate, cfg.input_channel)
                {
                    Ok(state) => Screen::Looper(state),
                    Err(err) => Screen::Settings(SettingsState::new(Some(err))),
                }
            }
            None => Screen::Settings(SettingsState::new(None)),
        }
    }
}

impl eframe::App for LooperPedalApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut next_screen = None;

        match &mut self.screen {
            Screen::Settings(settings) => {
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
                                .selected_text(format!(
                                    "Input {}",
                                    settings.selected_input_channel + 1
                                ))
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
                                let device_name = settings.devices[settings.selected_device].clone();
                                let sample_rate = settings.sample_rates[settings.selected_rate];
                                let input_channel = settings.selected_input_channel as u16;
                                match LooperPedalApp::try_start_looper(
                                    &device_name,
                                    sample_rate,
                                    input_channel,
                                ) {
                                    Ok(state) => {
                                        let _ = AppConfig {
                                            device_name,
                                            sample_rate,
                                            input_channel,
                                        }
                                        .save();
                                        next_screen = Some(Screen::Looper(state));
                                    }
                                    Err(err) => settings.error = Some(err),
                                }
                            }
                        });
                    });
            }
            Screen::Looper(state) => {
                // Long-press detection needs continuous frame updates, not
                // just repaints triggered by input events.
                ui.ctx().request_repaint();

                let space_down = ui.ctx().input(|i| i.key_down(egui::Key::Space));

                egui::Frame::default()
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        // Fixed top row - the gear stays put here
                        // regardless of the indicator's height changing
                        // below (e.g. the progress bar appearing).
                        ui.horizontal(|ui| {
                            ui.label("Looper Pedal");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("⚙").clicked() {
                                        next_screen =
                                            Some(Screen::Settings(SettingsState::new(None)));
                                    }
                                },
                            );
                        });
                        ui.add_space(8.0);

                        ui.vertical_centered(|ui| {
                            ui.spacing_mut().item_spacing.y = 10.0;

                            // Button and spacebar drive the exact same
                            // InputHandler, so they're always
                            // interchangeable and can't desync.
                            let button_response = ui.add_sized(
                                [90.0, 90.0],
                                egui::Button::new("Press").corner_radius(45),
                            );
                            let button_down = button_response.is_pointer_button_down_on();

                            match state
                                .input_handler
                                .update(space_down || button_down, Instant::now())
                            {
                                InputEvent::ShortPress => {
                                    state.state_machine.press();
                                    state.control.publish_state(state.state_machine.state());
                                }
                                InputEvent::LongPressClear => {
                                    state.state_machine.clear();
                                    state.control.publish_state(state.state_machine.state());
                                    state.control.request_clear();
                                }
                                InputEvent::None => {}
                            }

                            ui.add_space(4.0);
                            let (duration_secs, progress_fraction) =
                                state.control.loop_duration_and_progress(state.sample_rate);
                            ui::indicator::state_indicator(
                                ui,
                                state.state_machine.state(),
                                duration_secs,
                                progress_fraction,
                            );
                            ui.add_space(4.0);
                            ui.label("Space or button: cycle record/loop/stop  |  hold ~2s: clear");
                        });
                    });
            }
        }

        if let Some(screen) = next_screen {
            self.screen = screen;
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 300.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Looper Pedal",
        options,
        Box::new(|_cc| {
            Ok(Box::new(LooperPedalApp {
                screen: LooperPedalApp::initial_screen(),
            }))
        }),
    )
}
