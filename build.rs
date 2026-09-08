// Embeds the app icon into the executable as a Windows icon resource, so
// Explorer, the taskbar and Alt-Tab show it. The artwork itself is drawn
// by `src/ui/icon.rs`, included here rather than duplicated - the same
// code draws the window icon at runtime.
include!("src/ui/icon.rs");

/// Sizes Windows picks between: small list icons through to the large
/// Explorer tile.
const ICON_SIZES: [u32; 5] = [16, 32, 48, 64, 256];

fn main() {
    println!("cargo:rerun-if-changed=src/ui/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set for build scripts");
    let ico_path = std::path::Path::new(&out_dir).join("icon.ico");
    std::fs::write(&ico_path, ico_bytes(&ICON_SIZES)).expect("write icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(&ico_path.to_string_lossy());

    // Warn rather than fail: embedding needs rc.exe from the Windows SDK,
    // and a missing icon is no reason for the app not to build.
    if let Err(err) = resource.compile() {
        println!("cargo:warning=executable icon not embedded: {err}");
    }
}

/// An .ico holding every size: a header, one directory entry each, then
/// the images back to back.
fn ico_bytes(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = sizes.iter().map(|&s| (s, dib_image(s))).collect();

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // 1 = icon, not cursor
    ico.extend_from_slice(&(images.len() as u16).to_le_bytes());

    let mut offset = 6 + 16 * images.len() as u32;
    for (size, image) in &images {
        // 256 doesn't fit in the byte, and is written as 0 by convention.
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        ico.push(dimension);
        ico.push(dimension);
        ico.push(0); // palette size, 0 for true colour
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        ico.extend_from_slice(&(image.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += image.len() as u32;
    }

    for (_, image) in &images {
        ico.extend_from_slice(image);
    }
    ico
}

/// One icon image: a bitmap header, then bottom-up BGRA rows, then the
/// legacy 1-bit AND mask (left zeroed - the alpha channel is what
/// actually cuts the shape out, but the mask still has to be there).
fn dib_image(size: u32) -> Vec<u8> {
    let rgba = icon_rgba(size);
    let pixels_len = size * size * 4;
    let mask_len = size.div_ceil(32) * 4 * size;

    let mut image = Vec::with_capacity(40 + (pixels_len + mask_len) as usize);
    image.extend_from_slice(&40u32.to_le_bytes()); // header size
    image.extend_from_slice(&(size as i32).to_le_bytes());
    // Doubled: the header counts the colour rows and the mask rows.
    image.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    image.extend_from_slice(&1u16.to_le_bytes()); // planes
    image.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    image.extend_from_slice(&0u32.to_le_bytes()); // uncompressed
    image.extend_from_slice(&(pixels_len + mask_len).to_le_bytes());
    for _ in 0..4 {
        image.extend_from_slice(&0u32.to_le_bytes()); // resolution, palette
    }

    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            image.push(rgba[i + 2]);
            image.push(rgba[i + 1]);
            image.push(rgba[i]);
            image.push(rgba[i + 3]);
        }
    }
    image.resize(image.len() + mask_len as usize, 0);
    image
}
