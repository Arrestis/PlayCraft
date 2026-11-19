// Shared by the program and build.rs (included with `include!`).

/// Make the area outside the icon's rounded square transparent, with soft edges.
#[allow(dead_code)]
pub fn round_icon(img: &mut image::RgbaImage) {
    let (w, h) = img.dimensions();
    let s = w.min(h) as f32;
    let inset = s * 0.004;
    let radius = s * 0.16;
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let lo = inset + radius;
            let hi_x = w as f32 - inset - radius;
            let hi_y = h as f32 - inset - radius;
            let dx = if px < lo { lo - px } else if px > hi_x { px - hi_x } else { 0.0 };
            let dy = if py < lo { lo - py } else if py > hi_y { py - hi_y } else { 0.0 };
            let edge_x = px.min(w as f32 - px) - inset;
            let edge_y = py.min(h as f32 - py) - inset;
            let d = if dx > 0.0 && dy > 0.0 { radius - (dx * dx + dy * dy).sqrt() } else { edge_x.min(edge_y) };
            let a = (d + 0.5).clamp(0.0, 1.0);
            let p = img.get_pixel_mut(x, y);
            p[3] = (p[3] as f32 * a) as u8;
        }
    }
}

/// Decode the bundled icon picture at the given size with rounded corners.
#[allow(dead_code)]
pub fn icon_rgba(bytes: &[u8], size: u32) -> Option<image::RgbaImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let mut img = image::imageops::resize(&img, size, size, image::imageops::FilterType::Lanczos3);
    round_icon(&mut img);
    Some(img)
}
