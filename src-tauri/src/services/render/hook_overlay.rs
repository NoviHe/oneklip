use crate::error::{AppError, AppResult};
use crate::services::render::hook_style::HookStyle;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use std::path::Path;

fn parse_hex_rgba(hex: &str, alpha: u8) -> Rgba<u8> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() >= 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
        Rgba([r, g, b, alpha])
    } else {
        Rgba([255, 255, 255, alpha])
    }
}

fn draw_rounded_box(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    radius: i32,
    color: Rgba<u8>,
) {
    let r = radius.min((w as i32) / 2).min((h as i32) / 2).max(0);
    if r == 0 {
        draw_filled_rect_mut(img, Rect::at(x, y).of_size(w, h), color);
        return;
    }

    let inner_w = (w as i32 - 2 * r).max(1) as u32;
    let inner_h = (h as i32 - 2 * r).max(1) as u32;

    draw_filled_rect_mut(img, Rect::at(x + r, y).of_size(inner_w, h), color);
    draw_filled_rect_mut(img, Rect::at(x, y + r).of_size(w, inner_h), color);

    draw_filled_circle_mut(img, (x + r, y + r), r, color);
    draw_filled_circle_mut(img, (x + w as i32 - r - 1, y + r), r, color);
    draw_filled_circle_mut(img, (x + r, y + h as i32 - r - 1), r, color);
    draw_filled_circle_mut(
        img,
        (x + w as i32 - r - 1, y + h as i32 - r - 1),
        r,
        color,
    );
}

fn measure_text_width(font: &FontArc, scale: PxScale, text: &str) -> u32 {
    let scaled = font.as_scaled(scale);
    let mut width = 0.0_f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let gid = scaled.glyph_id(ch);
        if let Some(p) = prev {
            width += scaled.kern(p, gid);
        }
        width += scaled.h_advance(gid);
        prev = Some(gid);
    }
    width.ceil() as u32
}

fn wrap_text_lines(font: &FontArc, scale: PxScale, text: &str, max_width: u32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::from("MOMEN VIRAL")];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for w in words {
        let candidate = if current.is_empty() {
            w.to_string()
        } else {
            format!("{} {}", current, w)
        };
        if measure_text_width(font, scale, &candidate) <= max_width || current.is_empty() {
            current = candidate;
        } else {
            lines.push(current);
            current = w.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }

    lines.into_iter().take(3).collect()
}

pub fn render_hook_png(
    fonts_dir: &Path,
    hook_text: &str,
    hook_kicker: Option<&str>,
    hook_tag: Option<&str>,
    style: &HookStyle,
    out_png: &Path,
) -> AppResult<()> {
    if let Some(parent) = out_png.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let font_candidates = [
        fonts_dir.join("Poppins-Bold.ttf"),
        fonts_dir.join("Inter-Bold.ttf"),
        fonts_dir.join("theboldfont.ttf"),
    ];
    let font_bytes = font_candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .ok_or_else(|| {
            AppError::custom("font_missing", "File font TTF untuk hook overlay tidak ditemukan")
        })?;

    let font = FontArc::try_from_vec(font_bytes)
        .map_err(|_| AppError::custom("font_invalid", "Gagal memuat font TTF"))?;

    let canvas_w = 1080_u32;
    let canvas_h = 1920_u32;
    let mut img = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba([0, 0, 0, 0]));

    let headline_scale = PxScale::from(52.0);
    let kicker_scale = PxScale::from(30.0);
    let tag_scale = PxScale::from(24.0);

    let max_text_w = 820_u32;
    let lines = wrap_text_lines(&font, headline_scale, &hook_text.to_uppercase(), max_text_w);

    let line_height = 62_i32;
    let pad_x = 44_u32;
    let pad_y = 32_u32;

    let widest_line = lines
        .iter()
        .map(|l| measure_text_width(&font, headline_scale, l))
        .max()
        .unwrap_or(400);

    let card_w = (widest_line + pad_x * 2).clamp(480, 940);
    let card_h = (lines.len() as u32 * line_height as u32) + pad_y * 2;
    let card_x = ((canvas_w - card_w) / 2) as i32;
    let card_y = (style.y_position_ratio.clamp(0.08, 0.45) * canvas_h as f32).round() as i32;

    // 1. Draw soft drop shadow + border outline + main card
    draw_rounded_box(
        &mut img,
        card_x + 8,
        card_y + 10,
        card_w,
        card_h,
        28,
        Rgba([0, 0, 0, 175]),
    );
    draw_rounded_box(
        &mut img,
        card_x - 4,
        card_y - 4,
        card_w + 8,
        card_h + 8,
        30,
        Rgba([15, 23, 42, 255]),
    );
    let card_bg = parse_hex_rgba(&style.card_bg_color, 255);
    draw_rounded_box(&mut img, card_x, card_y, card_w, card_h, 26, card_bg);

    // 2. Draw headline text lines centered inside the card
    let text_col = parse_hex_rgba(&style.text_color, 255);
    for (idx, line) in lines.iter().enumerate() {
        let lw = measure_text_width(&font, headline_scale, line);
        let lx = card_x + ((card_w.saturating_sub(lw)) / 2) as i32;
        let ly = card_y + pad_y as i32 + (idx as i32 * line_height);
        draw_text_mut(&mut img, text_col, lx, ly, headline_scale, &font, line);
    }

    // 3. Draw top kicker pill badge overlapping the top edge of the card
    if let Some(kicker) = hook_kicker.filter(|k| !k.trim().is_empty()) {
        let kicker_up = kicker.trim().to_uppercase();
        let kw = measure_text_width(&font, kicker_scale, &kicker_up);
        let badge_w = (kw + 48).clamp(180, 700);
        let badge_h = 48_u32;
        let badge_x = ((canvas_w - badge_w) / 2) as i32;
        let badge_y = (card_y - (badge_h as i32 / 2) - 4).max(20);

        draw_rounded_box(
            &mut img,
            badge_x - 3,
            badge_y - 3,
            badge_w + 6,
            badge_h + 6,
            24,
            Rgba([0, 0, 0, 240]),
        );
        let kicker_bg = parse_hex_rgba(&style.kicker_bg_color, 255);
        draw_rounded_box(&mut img, badge_x, badge_y, badge_w, badge_h, 22, kicker_bg);

        let kicker_fg = parse_hex_rgba(&style.kicker_text_color, 255);
        let tx = badge_x + ((badge_w.saturating_sub(kw)) / 2) as i32;
        let ty = badge_y + 9;
        draw_text_mut(&mut img, kicker_fg, tx, ty, kicker_scale, &font, &kicker_up);
    }

    // 4. Draw bottom tag pill if provided
    if let Some(tag) = hook_tag.filter(|t| !t.trim().is_empty()) {
        let tag_up = format!("• {} •", tag.trim().to_uppercase());
        let tw = measure_text_width(&font, tag_scale, &tag_up);
        let pill_w = (tw + 36).clamp(140, 520);
        let pill_h = 36_u32;
        let pill_x = ((canvas_w - pill_w) / 2) as i32;
        let pill_y = card_y + card_h as i32 - (pill_h as i32 / 2) + 4;

        draw_rounded_box(
            &mut img,
            pill_x,
            pill_y,
            pill_w,
            pill_h,
            18,
            Rgba([15, 23, 42, 245]),
        );
        let tx = pill_x + ((pill_w.saturating_sub(tw)) / 2) as i32;
        let ty = pill_y + 6;
        draw_text_mut(
            &mut img,
            Rgba([250, 204, 21, 255]),
            tx,
            ty,
            tag_scale,
            &font,
            &tag_up,
        );
    }

    img.save(out_png).map_err(|e| {
        AppError::custom(
            "save_hook_png_failed",
            format!("Gagal menyimpan intro_hook.png: {}", e),
        )
    })?;

    Ok(())
}
