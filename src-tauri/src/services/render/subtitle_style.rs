use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleStyle {
    pub preset_id: String,
    pub font_family: String,
    pub font_size: u32,
    pub primary_color: String,
    pub highlight_color: String,
    pub outline_color: String,
    pub box_color: String,
    pub outline_width: f32,
    pub shadow_depth: f32,
    pub reveal_mode: String, // "chunk_karaoke" | "per_word" | "karaoke_sweep" | "cumulative" | "segment_static"
    pub animation: String,   // "pop" | "bump" | "slam" | "glow" | "neon" | "comic" | "word_box" | "fade"
    pub words_per_chunk: usize,
    pub vertical_position: f32, // 0.15 (top) .. 0.88 (bottom), default 0.74
    pub uppercase: bool,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self::from_preset("hormozi_pop")
    }
}

impl SubtitleStyle {
    pub fn builtin_presets() -> Vec<Self> {
        vec![
            Self::from_preset("hormozi_pop"),
            Self::from_preset("mrbeast_slam"),
            Self::from_preset("karaoke_box"),
            Self::from_preset("neon_cyber"),
            Self::from_preset("podcast_clean"),
            Self::from_preset("comic_punch"),
            Self::from_preset("glow_viral"),
            Self::from_preset("minimal_fade"),
            Self::from_preset("retro_gold"),
        ]
    }

    pub fn from_preset(preset_id: &str) -> Self {
        match preset_id {
            "mrbeast_slam" => Self {
                preset_id: "mrbeast_slam".into(),
                font_family: "The Bold Font".into(),
                font_size: 80,
                primary_color: "#FFFFFF".into(),
                highlight_color: "#00FF66".into(),
                outline_color: "#000000".into(),
                box_color: "#FF0044".into(),
                outline_width: 7.0,
                shadow_depth: 4.0,
                reveal_mode: "per_word".into(),
                animation: "slam".into(),
                words_per_chunk: 2,
                vertical_position: 0.72,
                uppercase: true,
            },
            "karaoke_box" => Self {
                preset_id: "karaoke_box".into(),
                font_family: "Poppins".into(),
                font_size: 68,
                primary_color: "#FFFFFF".into(),
                highlight_color: "#FFE600".into(),
                outline_color: "#000000".into(),
                box_color: "#7C3AED".into(),
                outline_width: 5.0,
                shadow_depth: 3.0,
                reveal_mode: "chunk_karaoke".into(),
                animation: "word_box".into(),
                words_per_chunk: 3,
                vertical_position: 0.74,
                uppercase: true,
            },
            "neon_cyber" => Self {
                preset_id: "neon_cyber".into(),
                font_family: "Poppins".into(),
                font_size: 70,
                primary_color: "#E2E8F0".into(),
                highlight_color: "#00F5D4".into(),
                outline_color: "#0F172A".into(),
                box_color: "#00BBF9".into(),
                outline_width: 5.0,
                shadow_depth: 2.0,
                reveal_mode: "chunk_karaoke".into(),
                animation: "neon".into(),
                words_per_chunk: 3,
                vertical_position: 0.74,
                uppercase: true,
            },
            "podcast_clean" => Self {
                preset_id: "podcast_clean".into(),
                font_family: "Inter".into(),
                font_size: 64,
                primary_color: "#FFFFFF".into(),
                highlight_color: "#38BDF8".into(),
                outline_color: "#000000".into(),
                box_color: "#1E293B".into(),
                outline_width: 4.5,
                shadow_depth: 2.5,
                reveal_mode: "chunk_karaoke".into(),
                animation: "bump".into(),
                words_per_chunk: 4,
                vertical_position: 0.76,
                uppercase: false,
            },
            "comic_punch" => Self {
                preset_id: "comic_punch".into(),
                font_family: "Lilita One".into(),
                font_size: 78,
                primary_color: "#FFFBEB".into(),
                highlight_color: "#F97316".into(),
                outline_color: "#000000".into(),
                box_color: "#EF4444".into(),
                outline_width: 6.5,
                shadow_depth: 4.0,
                reveal_mode: "chunk_karaoke".into(),
                animation: "comic".into(),
                words_per_chunk: 3,
                vertical_position: 0.73,
                uppercase: true,
            },
            "glow_viral" => Self {
                preset_id: "glow_viral".into(),
                font_family: "Poppins".into(),
                font_size: 72,
                primary_color: "#FFFFFF".into(),
                highlight_color: "#F43F5E".into(),
                outline_color: "#09090B".into(),
                box_color: "#F43F5E".into(),
                outline_width: 5.5,
                shadow_depth: 3.0,
                reveal_mode: "karaoke_sweep".into(),
                animation: "glow".into(),
                words_per_chunk: 3,
                vertical_position: 0.74,
                uppercase: true,
            },
            "minimal_fade" => Self {
                preset_id: "minimal_fade".into(),
                font_family: "Lato".into(),
                font_size: 60,
                primary_color: "#F8FAFC".into(),
                highlight_color: "#A855F7".into(),
                outline_color: "#000000".into(),
                box_color: "#000000".into(),
                outline_width: 3.5,
                shadow_depth: 2.0,
                reveal_mode: "cumulative".into(),
                animation: "fade".into(),
                words_per_chunk: 4,
                vertical_position: 0.78,
                uppercase: false,
            },
            "retro_gold" => Self {
                preset_id: "retro_gold".into(),
                font_family: "The Bold Font".into(),
                font_size: 74,
                primary_color: "#FEF3C7".into(),
                highlight_color: "#F59E0B".into(),
                outline_color: "#1C1917".into(),
                box_color: "#B45309".into(),
                outline_width: 6.0,
                shadow_depth: 3.5,
                reveal_mode: "chunk_karaoke".into(),
                animation: "pop".into(),
                words_per_chunk: 3,
                vertical_position: 0.74,
                uppercase: true,
            },
            _ => Self {
                preset_id: "hormozi_pop".into(),
                font_family: "The Bold Font".into(),
                font_size: 74,
                primary_color: "#FFFFFF".into(),
                highlight_color: "#FFE600".into(),
                outline_color: "#000000".into(),
                box_color: "#22C55E".into(),
                outline_width: 6.0,
                shadow_depth: 3.5,
                reveal_mode: "chunk_karaoke".into(),
                animation: "pop".into(),
                words_per_chunk: 3,
                vertical_position: 0.74,
                uppercase: true,
            },
        }
    }
}

/// Converts a `#RRGGBB` hex string into ASS `&H00BBGGRR` format (or override `&HBBGGRR&`).
pub fn hex_to_ass_color(hex: &str, alpha_byte: u8) -> String {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() >= 6 {
        let r = &clean[0..2];
        let g = &clean[2..4];
        let b = &clean[4..6];
        format!("&H{:02X}{}{}{}", alpha_byte, b.to_uppercase(), g.to_uppercase(), r.to_uppercase())
    } else {
        format!("&H{:02X}FFFFFF", alpha_byte)
    }
}

pub fn hex_to_ass_override_bgr(hex: &str) -> String {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() >= 6 {
        let r = &clean[0..2];
        let g = &clean[2..4];
        let b = &clean[4..6];
        format!("&H{}{}{}&", b.to_uppercase(), g.to_uppercase(), r.to_uppercase())
    } else {
        "&HFFFFFF&".to_string()
    }
}
