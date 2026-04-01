use image::{Rgba, RgbaImage};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::RwLock;

const ICON_SIZE: u32 = 144;
const ICON_PADDING: u32 = 32;

const KNOB_WIDTH: u32 = 144;
const KNOB_HEIGHT: u32 = 72;

const ICONIFY_API: &str = "https://api.iconify.design";

const BG_COLOR: Rgba<u8> = Rgba([40, 40, 50, 255]);
const KNOB_BG: Rgba<u8> = Rgba([0, 0, 0, 255]);
const ICON_COLOR: Rgba<u8> = Rgba([220, 220, 225, 255]);

pub fn create_unlocked_button_with_icon(icon_b64: &str, is_light: bool) -> String {
    let overlay_color = if is_light {
        Rgba([120, 120, 150, 80])
    } else {
        Rgba([60, 60, 80, 80])
    };

    if let Some(icon_data) = icon_b64.strip_prefix("data:image/png;base64,") {
        if let Ok(decoded) =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, icon_data)
        {
            if let Ok(icon_img) = image::load_from_memory(&decoded) {
                let mut img = icon_img.to_rgba8();

                for y in 0..img.height() {
                    for x in 0..img.width() {
                        let pixel = img.get_pixel(x, y);
                        if pixel.0[3] > 0 {
                            let r = ((pixel.0[0] as u32 * (255 - overlay_color.0[3] as u32)
                                + overlay_color.0[0] as u32 * overlay_color.0[3] as u32)
                                / 255) as u8;
                            let g = ((pixel.0[1] as u32 * (255 - overlay_color.0[3] as u32)
                                + overlay_color.0[1] as u32 * overlay_color.0[3] as u32)
                                / 255) as u8;
                            let b = ((pixel.0[2] as u32 * (255 - overlay_color.0[3] as u32)
                                + overlay_color.0[2] as u32 * overlay_color.0[3] as u32)
                                / 255) as u8;
                            img.put_pixel(x, y, Rgba([r, g, b, pixel.0[3]]));
                        }
                    }
                }

                let mut buf = Cursor::new(Vec::new());
                img.write_to(&mut buf, image::ImageFormat::Png)
                    .expect("Failed to write PNG");

                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    buf.into_inner(),
                );

                return format!("data:image/png;base64,{}", encoded);
            }
        }
    }

    icon_b64.to_string()
}

pub fn create_blank_button() -> String {
    let img = RgbaImage::from_pixel(ICON_SIZE, ICON_SIZE, KNOB_BG);

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("Failed to write PNG");

    let encoded =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    format!("data:image/png;base64,{}", encoded)
}

pub fn create_blank_knob() -> String {
    let img = RgbaImage::from_pixel(KNOB_WIDTH, KNOB_HEIGHT, KNOB_BG);

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("Failed to write PNG");

    let encoded =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    format!("data:image/png;base64,{}", encoded)
}

static REQWEST_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
static ICON_CACHE: std::sync::OnceLock<Arc<RwLock<HashMap<String, String>>>> =
    std::sync::OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    REQWEST_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .user_agent("OpenDeck-Audio-Control/0.1")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn get_cache() -> Arc<RwLock<HashMap<String, String>>> {
    ICON_CACHE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

fn apply_style(icon: &RgbaImage) -> RgbaImage {
    let mut result = RgbaImage::from_pixel(ICON_SIZE, ICON_SIZE, BG_COLOR);

    let icon_actual_size = ICON_SIZE - ICON_PADDING * 2;

    let resized_icon = image::imageops::resize(
        icon,
        icon_actual_size,
        icon_actual_size,
        image::imageops::FilterType::CatmullRom,
    );

    let mut tinted_icon = tint_icon(&resized_icon, ICON_COLOR);
    smooth_edges(&mut tinted_icon);
    image::imageops::overlay(
        &mut result,
        &tinted_icon,
        ICON_PADDING as i64,
        ICON_PADDING as i64,
    );

    result
}

fn smooth_edges(img: &mut RgbaImage) {
    let width = img.width();
    let height = img.height();
    let mut result = img.clone();

    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let pixel = img.get_pixel(x, y);
            if pixel.0[3] > 0 && pixel.0[3] < 255 {
                continue;
            }

            let mut neighbors_alpha = 0u32;
            let mut count = 0u32;

            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = (x as i32 + dx) as u32;
                    let ny = (y as i32 + dy) as u32;
                    if nx < width && ny < height {
                        let n = img.get_pixel(nx, ny);
                        if n.0[3] > 0 {
                            neighbors_alpha += n.0[3] as u32;
                            count += 1;
                        }
                    }
                }
            }

            if count > 0 && pixel.0[3] == 255 {
                let avg_alpha = neighbors_alpha / count;
                if avg_alpha < 200 {
                    let blend = 0.7;
                    let new_alpha = ((255.0 * blend) + (avg_alpha as f32 * (1.0 - blend))) as u8;
                    result.put_pixel(x, y, Rgba([pixel.0[0], pixel.0[1], pixel.0[2], new_alpha]));
                }
            }
        }
    }

    *img = result;
}

fn tint_icon(icon: &RgbaImage, tint: Rgba<u8>) -> RgbaImage {
    let mut result = icon.clone();
    for pixel in result.pixels_mut() {
        let a = pixel.0[3];
        if a > 0 {
            let alpha = a as f32 / 255.0;
            let r = (tint.0[0] as f32 * alpha).min(255.0) as u8;
            let g = (tint.0[1] as f32 * alpha).min(255.0) as u8;
            let b = (tint.0[2] as f32 * alpha).min(255.0) as u8;
            *pixel = Rgba([r, g, b, a]);
        }
    }
    result
}

async fn fetch_icon_from_iconify(query: &str) -> Option<String> {
    let encoded_query = urlencoding::encode(query);
    let search_url = format!("{}/search?query={}&limit=5", ICONIFY_API, encoded_query);

    let search_response = get_client().get(&search_url).send().await.ok()?;

    if !search_response.status().is_success() {
        return None;
    }

    let search_json: serde_json::Value = search_response.json().await.ok()?;

    let icons = search_json.get("icons").and_then(|i| i.as_array())?;
    if icons.is_empty() {
        return None;
    }

    let first_icon = icons[0].as_str().unwrap_or("");
    let icon_path = first_icon.replace(':', "/");
    let icon_url = format!("{}/{}.svg?width=256&height=256", ICONIFY_API, icon_path);

    let icon_response = get_client().get(&icon_url).send().await.ok()?;

    if !icon_response.status().is_success() {
        return None;
    }

    let svg_data = icon_response.bytes().await.ok()?;
    process_icon_bytes(&svg_data, query)
}

fn process_icon_bytes(data: &[u8], _app_name: &str) -> Option<String> {
    let svg_str = std::str::from_utf8(data).ok()?;

    let tree = resvg::usvg::Tree::from_str(svg_str, &resvg::usvg::Options::default()).ok()?;

    let size = tree.size();
    let width = size.width() as u32;
    let height = size.height() as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );

    let rgba_data = pixmap.data();
    let img = image::RgbaImage::from_raw(width, height, rgba_data.to_vec())?;

    let resized = image::imageops::resize(
        &img,
        ICON_SIZE - ICON_PADDING * 2,
        ICON_SIZE - ICON_PADDING * 2,
        image::imageops::FilterType::Lanczos3,
    );

    let styled = apply_style(&resized);

    let mut buf = Cursor::new(Vec::new());
    styled.write_to(&mut buf, image::ImageFormat::Png).ok()?;

    let encoded =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    Some(format!("data:image/png;base64,{}", encoded))
}

pub async fn get_app_icon_base64(app_name: &str) -> Option<String> {
    let cache_key = app_name.to_lowercase();

    {
        let cache = get_cache();
        let cache_read = cache.read().await;
        if let Some(cached) = cache_read.get(&cache_key) {
            return Some(cached.clone());
        }
    }

    let icon = if let Some(icon) = fetch_icon_from_iconify(app_name).await {
        Some(icon)
    } else {
        let lower = app_name.to_lowercase();
        let first_word = lower.split_whitespace().next().unwrap_or(&lower);
        if first_word != app_name {
            fetch_icon_from_iconify(first_word).await
        } else {
            None
        }
    };

    let result = match icon {
        Some(i) => i,
        None => fetch_icon_from_iconify("fluent:app-generic-48-filled").await?,
    };

    {
        let cache = get_cache();
        let mut cache_write = cache.write().await;
        cache_write.insert(cache_key, result.clone());
    }

    Some(result)
}

pub fn apply_muted_stroke(icon_data: &str) -> String {
    if !icon_data.starts_with("data:image/png;base64,") {
        return icon_data.to_string();
    }

    let b64 = &icon_data["data:image/png;base64,".len()..];
    let bytes = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
        Ok(b) => b,
        Err(_) => return icon_data.to_string(),
    };

    let mut img: RgbaImage = match image::load_from_memory(&bytes) {
        Ok(i) => i.to_rgba8(),
        Err(_) => return icon_data.to_string(),
    };

    for y in 0..img.height() {
        for x in 0..img.width() {
            let pixel = img.get_pixel(x, y);
            if pixel.0[3] > 0 {
                let gray = (pixel.0[0] as f32 * 0.3
                    + pixel.0[1] as f32 * 0.59
                    + pixel.0[2] as f32 * 0.11) as u8;
                let muted_gray = (gray as f32 * 0.5) as u8;
                img.put_pixel(x, y, Rgba([muted_gray, muted_gray, muted_gray, pixel.0[3]]));
            }
        }
    }

    let slash_color = Rgba([255, 0, 0, 255]);
    let thickness = 2i32;
    let w = img.width() as f32;
    let h = img.height() as f32;
    let center_x = w / 2.0;
    let center_y = h / 2.0;
    let line_length = w.min(h) * 0.75;
    let angle = 35.0_f32.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();

    for t in (-line_length / 2.0) as i32..=(line_length / 2.0) as i32 {
        let t = t as f32;
        let x = (center_x + t * cos_a) as i32;
        let y = (center_y + t * sin_a) as i32;
        if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
            for dx in -thickness..=thickness {
                for dy in -thickness..=thickness {
                    let tx = x + dx;
                    let ty = y + dy;
                    if tx >= 0 && tx < w as i32 && ty >= 0 && ty < h as i32 {
                        img.put_pixel(tx as u32, ty as u32, slash_color);
                    }
                }
            }
        }
    }

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("Failed to write PNG");

    let encoded =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    format!("data:image/png;base64,{}", encoded)
}

pub fn create_knob_display(_name: &str, volume: u32, muted: bool) -> String {
    let mut img = RgbaImage::from_pixel(KNOB_WIDTH, KNOB_HEIGHT, KNOB_BG);

    let bar_y = 50u32;
    let bar_height = 12u32;
    let bar_start = 8u32;
    let bar_end = KNOB_WIDTH - 8;
    let bar_width = bar_end - bar_start;
    let bar_radius = 6u32;

    let bar_bg = Rgba([80, 80, 95, 255]);
    draw_rounded_rect(
        &mut img, bar_start, bar_y, bar_width, bar_height, bar_radius, bar_bg,
    );

    if muted {
        let muted_bar = Rgba([60, 60, 70, 255]);
        draw_rounded_rect(
            &mut img, bar_start, bar_y, bar_width, bar_height, bar_radius, muted_bar,
        );
    } else {
        let filled_width = (bar_width as f32 * volume as f32 / 100.0).min(bar_width as f32) as u32;

        if filled_width > 0 {
            draw_rounded_rect(
                &mut img,
                bar_start,
                bar_y,
                filled_width,
                bar_height,
                bar_radius,
                ICON_COLOR,
            );
        }
    }

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("Failed to write PNG");

    let encoded =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, buf.into_inner());

    format!("data:image/png;base64,{}", encoded)
}

fn draw_rounded_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, r: u32, color: Rgba<u8>) {
    let r = r.min(w / 2).min(h / 2);

    for py in y..(y + h) {
        for px in x..(x + w) {
            let dx = if px < x + r {
                (x + r - px) as i32
            } else if px > x + w - r {
                (px - (x + w - r)) as i32
            } else {
                0
            };

            let dy = if py < y + r {
                (y + r - py) as i32
            } else if py > y + h - r {
                (py - (y + h - r)) as i32
            } else {
                0
            };

            let inside = if dx == 0 && dy == 0 {
                true
            } else if dx > 0 && dy > 0 {
                (dx * dx + dy * dy) <= (r * r) as i32
            } else {
                true
            };

            if inside {
                img.put_pixel(px, py, color);
            }
        }
    }
}
