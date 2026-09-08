// The app icon, drawn in code rather than shipped as an image file: a
// green loop arrow - a ring with a gap and an arrowhead, so it reads as
// "loop" rather than as a target - around a red record dot, on a dark
// rounded square.
//
// Drawing it needs no image-decoding dependency and no binary asset in
// the repo, and it renders at whatever size is asked for - the window
// wants one, the executable's icon resource wants several. Keep this
// file free of any crate or third-party imports: `build.rs` `include!`s
// it to generate the `.ico`, so it has to stand alone.

/// Sub-samples per axis, per pixel. The shapes are curved and the
/// smallest icon size is 16px, so they need smoothing.
const SUPERSAMPLE: u32 = 4;

const CORNER_RADIUS: f32 = 0.22;
const RING_RADIUS: f32 = 0.30;
const RING_HALF_WIDTH: f32 = 0.055;
const DOT_RADIUS: f32 = 0.105;

/// Where the loop's arc begins and how far it sweeps, in radians from 3
/// o'clock. Angles increase clockwise on screen, because y grows
/// downward. The 60 degrees left over is the gap the arrowhead sits in,
/// at the top.
const ARC_START: f32 = 310.0 / 180.0 * std::f32::consts::PI;
const ARC_SWEEP: f32 = 300.0 / 180.0 * std::f32::consts::PI;
/// Comfortably wider than the stroke, or it doesn't read as an arrow.
const HEAD_LENGTH: f32 = 0.115;
const HEAD_HALF_WIDTH: f32 = 0.095;

const BACKGROUND: [f32; 3] = [30.0, 33.0, 38.0];
/// The same green and red the state indicator uses.
const RING: [f32; 3] = [40.0, 200.0, 40.0];
const DOT: [f32; 3] = [220.0, 60.0, 50.0];

/// `size` x `size` pixels of straight (non-premultiplied) RGBA.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let samples_per_pixel = (SUPERSAMPLE * SUPERSAMPLE) as f32;

    for y in 0..size {
        for x in 0..size {
            let mut color = [0.0f32; 3];
            let mut covered = 0.0f32;

            for sub_y in 0..SUPERSAMPLE {
                for sub_x in 0..SUPERSAMPLE {
                    let point = (sub_position(x, sub_x, size), sub_position(y, sub_y, size));
                    if let Some(sample) = color_at(point) {
                        for (channel, value) in color.iter_mut().zip(sample) {
                            *channel += value;
                        }
                        covered += 1.0;
                    }
                }
            }

            // Colors average over the samples that actually hit
            // something; alpha is what fraction of them did.
            if covered > 0.0 {
                for channel in color.iter_mut() {
                    *channel /= covered;
                }
            }
            rgba.extend(color.iter().map(|c| c.round() as u8));
            rgba.push((255.0 * covered / samples_per_pixel).round() as u8);
        }
    }

    rgba
}

/// Where one sub-sample sits, as a 0.0-1.0 fraction across the icon.
fn sub_position(pixel: u32, sub: u32, size: u32) -> f32 {
    let offset = (sub as f32 + 0.5) / SUPERSAMPLE as f32;
    (pixel as f32 + offset) / size as f32
}

/// The topmost shape covering this point, or `None` where the icon is
/// transparent.
fn color_at((x, y): (f32, f32)) -> Option<[f32; 3]> {
    let (dx, dy) = (x - 0.5, y - 0.5);
    let from_center = (dx * dx + dy * dy).sqrt();

    if from_center <= DOT_RADIUS {
        Some(DOT)
    } else if on_arc(dx, dy, from_center) || in_arrow_head(x, y) || in_tail_cap(x, y) {
        Some(RING)
    } else if in_rounded_square(x, y) {
        Some(BACKGROUND)
    } else {
        None
    }
}

/// The loop itself: the ring, minus the gap left for the arrowhead.
fn on_arc(dx: f32, dy: f32, from_center: f32) -> bool {
    if (from_center - RING_RADIUS).abs() > RING_HALF_WIDTH {
        return false;
    }
    let angle = dy.atan2(dx);
    let swept = (angle - ARC_START).rem_euclid(std::f32::consts::TAU);
    swept <= ARC_SWEEP
}

/// The arrowhead: a triangle at the end of the arc, pointing the way the
/// loop travels, so the icon reads as going round rather than just being
/// a broken circle.
fn in_arrow_head(x: f32, y: f32) -> bool {
    let (sin_end, cos_end) = (ARC_START + ARC_SWEEP).sin_cos();
    let radial = (cos_end, sin_end);
    let along = (-sin_end, cos_end);
    let end = (0.5 + radial.0 * RING_RADIUS, 0.5 + radial.1 * RING_RADIUS);

    in_triangle(
        (x, y),
        (end.0 + along.0 * HEAD_LENGTH, end.1 + along.1 * HEAD_LENGTH),
        (
            end.0 + radial.0 * HEAD_HALF_WIDTH,
            end.1 + radial.1 * HEAD_HALF_WIDTH,
        ),
        (
            end.0 - radial.0 * HEAD_HALF_WIDTH,
            end.1 - radial.1 * HEAD_HALF_WIDTH,
        ),
    )
}

/// Rounds off the arc's other end, which would otherwise stop in a blunt
/// diagonal cut.
fn in_tail_cap(x: f32, y: f32) -> bool {
    let (sin_start, cos_start) = ARC_START.sin_cos();
    let tail = (0.5 + cos_start * RING_RADIUS, 0.5 + sin_start * RING_RADIUS);
    ((x - tail.0).powi(2) + (y - tail.1).powi(2)).sqrt() <= RING_HALF_WIDTH
}

/// Inside if the point falls on the same side of all three edges.
fn in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let side = |from: (f32, f32), to: (f32, f32)| {
        (to.0 - from.0) * (p.1 - from.1) - (to.1 - from.1) * (p.0 - from.0)
    };
    let (ab, bc, ca) = (side(a, b), side(b, c), side(c, a));
    (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
}

/// A rounded square filling the whole icon: distance to the inner box,
/// measured only where the point is outside it on that axis.
fn in_rounded_square(x: f32, y: f32) -> bool {
    let half_flat = 0.5 - CORNER_RADIUS;
    let past_x = ((x - 0.5).abs() - half_flat).max(0.0);
    let past_y = ((y - 0.5).abs() - half_flat).max(0.0);
    (past_x * past_x + past_y * past_y).sqrt() <= CORNER_RADIUS
}

#[cfg(test)]
#[path = "icon_tests.rs"]
mod tests;
