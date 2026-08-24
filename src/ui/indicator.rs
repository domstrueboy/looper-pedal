use crate::audio::state_machine::LoopState;

/// Icon for the main press button, reflecting what a short press does
/// next from the current state (record / finish-and-loop / pause /
/// resume) - or a trash icon while a long-press-clear is actively being
/// held, confirming the clear that just fired.
pub fn press_button_icon(state: LoopState, long_press_active: bool) -> &'static str {
    if long_press_active {
        return "🗑";
    }
    match state {
        LoopState::Idle => "⏺",
        LoopState::Recording => "⏹",
        LoopState::Looping => "⏸",
        LoopState::Stopped => "▶",
    }
}

/// Colored state circle + label + loop duration / progress, per the
/// visual-feedback table in PLAN.md. `loop_duration_secs` is the recorded
/// loop's length in seconds (0.0 if empty); `progress_fraction` is the
/// current playback position through the loop (0.0-1.0), only meaningful
/// while Looping.
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
        LoopState::Stopped => (egui::Color32::from_rgb(230, 170, 30), "Stopped"),
    };

    ui.horizontal(|ui| {
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());

        let radius = if state == LoopState::Recording {
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
        LoopState::Looping | LoopState::Stopped => {
            ui.label(format!("{loop_duration_secs:.1}s loop"));
            // Frozen at the last playback position while Stopped, rather
            // than disappearing - it's still meaningful (shows where
            // playback will resume from).
            ui.add(egui::ProgressBar::new(progress_fraction));
        }
    }
}
