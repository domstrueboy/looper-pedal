use super::*;

const SIZE: u32 = 64;

/// The RGBA of one pixel, by fraction across the icon.
fn pixel_at(rgba: &[u8], x_fraction: f32, y_fraction: f32) -> [u8; 4] {
    let x = (x_fraction * SIZE as f32) as usize;
    let y = (y_fraction * SIZE as f32) as usize;
    let i = (y * SIZE as usize + x) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

#[test]
fn is_rgba_at_the_requested_size() {
    for size in [16, 32, 256] {
        assert_eq!(icon_rgba(size).len(), (size * size * 4) as usize);
    }
}

#[test]
fn center_is_the_opaque_record_dot() {
    let rgba = icon_rgba(SIZE);
    let [r, g, b, a] = pixel_at(&rgba, 0.5, 0.5);
    assert_eq!(a, 255);
    assert!(r > g && r > b, "expected a red dot, got {r},{g},{b}");
}

#[test]
fn the_ring_is_green() {
    let rgba = icon_rgba(SIZE);
    // Straight left of center, on the ring's centerline.
    let [r, g, b, a] = pixel_at(&rgba, 0.5 - RING_RADIUS, 0.5);
    assert_eq!(a, 255);
    assert!(g > r && g > b, "expected a green ring, got {r},{g},{b}");
}

#[test]
fn between_ring_and_dot_is_the_background() {
    let rgba = icon_rgba(SIZE);
    let gap = (DOT_RADIUS + RING_RADIUS - RING_HALF_WIDTH) / 2.0;
    let [r, g, b, a] = pixel_at(&rgba, 0.5 - gap, 0.5);
    assert_eq!(a, 255);
    assert!(
        r < 60 && g < 60 && b < 60,
        "expected the dark background, got {r},{g},{b}"
    );
}

/// The pixel at a polar position, in fractions of the icon.
fn polar(rgba: &[u8], degrees: f32, radius: f32) -> [u8; 4] {
    let radians = degrees.to_radians();
    pixel_at(
        rgba,
        0.5 + radius * radians.cos(),
        0.5 + radius * radians.sin(),
    )
}

fn is_green([r, g, b, a]: [u8; 4]) -> bool {
    a == 255 && g > r && g > b
}

#[test]
fn the_loop_is_open_at_the_top() {
    let rgba = icon_rgba(SIZE);

    // Past the arrowhead, where the arc has stopped.
    let [r, g, b, a] = polar(&rgba, 295.0, RING_RADIUS);
    assert_eq!(a, 255);
    assert!(
        r < 60 && g < 60 && b < 60,
        "expected the gap, got {r},{g},{b}"
    );

    // Closed the rest of the way round.
    for degrees in [0.0, 90.0, 180.0, 225.0] {
        assert!(
            is_green(polar(&rgba, degrees, RING_RADIUS)),
            "arc should be green at {degrees} degrees"
        );
    }
}

#[test]
fn the_arrowhead_flares_past_the_stroke() {
    let rgba = icon_rgba(SIZE);
    let beyond = RING_RADIUS + RING_HALF_WIDTH * 1.4;

    // Somewhere in the gap the head sticks out wider than the arc...
    assert!(
        (240..=290)
            .step_by(5)
            .any(|d| is_green(polar(&rgba, d as f32, beyond))),
        "arrowhead should reach past the stroke"
    );
    // ...and nowhere along the plain arc does anything.
    assert!(
        !(0..=180)
            .step_by(5)
            .any(|d| is_green(polar(&rgba, d as f32, beyond))),
        "the arc itself should not"
    );
}

#[test]
fn corners_are_transparent() {
    let rgba = icon_rgba(SIZE);
    for (x, y) in [(0.0, 0.0), (0.99, 0.0), (0.0, 0.99), (0.99, 0.99)] {
        assert_eq!(pixel_at(&rgba, x, y)[3], 0, "corner {x},{y} should be cut");
    }
}

#[test]
fn edge_midpoints_are_filled() {
    let rgba = icon_rgba(SIZE);
    for (x, y) in [(0.5, 0.01), (0.5, 0.99), (0.01, 0.5), (0.99, 0.5)] {
        assert_eq!(
            pixel_at(&rgba, x, y)[3],
            255,
            "edge {x},{y} should be solid"
        );
    }
}
