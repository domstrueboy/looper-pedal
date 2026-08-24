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
use ui::settings::{SettingsAction, SettingsState};

struct LooperState {
    control: Arc<SharedControl>,
    state_machine: LoopStateMachine,
    input_handler: InputHandler,
    // Latches true the moment the button is pressed and stays true until
    // the pointer button is released globally, regardless of whether the
    // pointer drifts off the button's rect mid-hold - egui's own hover
    // tracking would otherwise read as a release the instant the cursor
    // leaves the widget, even with the mouse button still down, which is
    // exactly what broke long-press-clear from the on-screen button
    // (spacebar has no such issue - key_down has no positional component).
    button_held: bool,
    sample_rate: u32,
    _input_stream: cpal::Stream,
    _output_stream: cpal::Stream,
}

impl LooperState {
    /// Applies an input event, always publishing the resulting state to
    /// the audio thread in the same step - keeps that pairing from being
    /// duplicated (and potentially forgotten) at each call site.
    fn apply(&mut self, event: InputEvent) {
        match event {
            InputEvent::ShortPress => {
                self.state_machine.press();
                self.control.publish_state(self.state_machine.state());
            }
            InputEvent::LongPressClear => {
                self.state_machine.clear();
                self.control.publish_state(self.state_machine.state());
                self.control.request_clear();
            }
            InputEvent::None => {}
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
            button_held: false,
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
                if let Some(SettingsAction::Start {
                    device_name,
                    sample_rate,
                    input_channel,
                }) = ui::settings::render(ui, settings)
                {
                    match LooperPedalApp::try_start_looper(&device_name, sample_rate, input_channel)
                    {
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
            }
            Screen::Looper(state) => {
                // Long-press detection needs continuous frame updates, not
                // just repaints triggered by input events.
                ui.ctx().request_repaint();

                // Underrun counts are accumulated lock-free on the audio
                // threads and drained here instead of being logged from
                // inside the callbacks themselves (stdio isn't real-time
                // safe).
                let (input_underruns, output_underruns) = state.control.take_underrun_counts();
                if input_underruns > 0 {
                    eprintln!("input stream fell behind {input_underruns} time(s): try increasing latency");
                }
                if output_underruns > 0 {
                    eprintln!("output stream fell behind {output_underruns} time(s): try increasing latency");
                }

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
                            let icon = ui::indicator::press_button_icon(
                                state.state_machine.state(),
                                state.input_handler.is_long_press_active(),
                            );
                            let button_response = ui.add_sized(
                                [90.0, 90.0],
                                egui::Button::new(egui::RichText::new(icon).size(32.0))
                                    .corner_radius(45),
                            );

                            // Latched, not just `is_pointer_button_down_on()`
                            // - see the `button_held` field comment.
                            if button_response.is_pointer_button_down_on() {
                                state.button_held = true;
                            }
                            if !ui.input(|i| i.pointer.primary_down()) {
                                state.button_held = false;
                            }

                            let event = state
                                .input_handler
                                .update(space_down || state.button_held, Instant::now());
                            state.apply(event);

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
