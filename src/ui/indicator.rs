use crate::audio::state_machine::LoopState;

/// Icon for the main button: what a short press does next, or a trash icon
/// while a long-press-clear is being held.
pub fn press_button_icon(state: LoopState, long_press_active: bool) -> &'static str {
    if long_press_active {
        return "🗑";
    }
    match state {
        LoopState::Idle => "⏺",
        LoopState::Recording => "⏹",
        LoopState::Looping | LoopState::Overdubbing => "⏸",
        LoopState::Stopped => "▶",
    }
}

/// Colored state circle, label, and loop duration / progress.
/// `loop_duration_secs` is 0.0 when empty; `progress_fraction` (0.0-1.0) is
/// only meaningful once something is recorded.
pub fn state_indicator(
    ui: &mut egui::Ui,
    state: LoopState,
    loop_duration_secs: f32,
    progress_fraction: f32,
) {
    let (color, label) = match state {
        LoopState::Idle => (egui::Color32::GRAY, "Empty"),
        LoopState::Recording => (egui::Color32::RED, "Recording"),
        LoopState::Looping => (egui::Color32::from_rgb(40, 200, 40), "Looping"),
        LoopState::Overdubbing => (egui::Color32::from_rgb(255, 110, 40), "Overdubbing"),
        LoopState::Stopped => (egui::Color32::from_rgb(230, 170, 30), "Stopped"),
    };

    ui.horizontal(|ui| {
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());

        let radius = if matches!(state, LoopState::Recording | LoopState::Overdubbing) {
            // Gentle pulse while recording.
            let t = ui.input(|i| i.time) as f32;
            5.5 + 2.0 * (t * 3.0).sin().abs()
        } else {
            7.0
        };
        ui.painter().circle_filled(rect.center(), radius, color);

        ui.label(label);
    });

    match state {
        LoopState::Idle => {}
        LoopState::Recording => {
            ui.label(format!("{loop_duration_secs:.1}s"));
        }
        LoopState::Looping | LoopState::Stopped | LoopState::Overdubbing => {
            ui.label(format!("{loop_duration_secs:.1}s loop"));
            // Frozen at the last position while Stopped rather than
            // vanishing - it shows where playback will resume.
            ui.add(egui::ProgressBar::new(progress_fraction));
        }
    }
}
