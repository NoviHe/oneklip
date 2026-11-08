use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStyle {
    pub enabled: bool,
    pub preset: String, // "punchline" | "classic" | "comic" | "sticker"
    pub duration_seconds: f32,
    pub card_bg_color: String,
    pub text_color: String,
    pub kicker_bg_color: String,
    pub kicker_text_color: String,
    pub y_position_ratio: f32, // default 0.14 (top third of 1080x1920)
}

impl Default for HookStyle {
    fn default() -> Self {
        Self::from_preset("punchline")
    }
}

impl HookStyle {
    pub fn from_preset(preset: &str) -> Self {
        match preset {
            "classic" => Self {
                enabled: true,
                preset: "classic".into(),
                duration_seconds: 3.2,
                card_bg_color: "#FFFFFF".into(),
                text_color: "#0F172A".into(),
                kicker_bg_color: "#EF4444".into(),
                kicker_text_color: "#FFFFFF".into(),
                y_position_ratio: 0.14,
            },
            "comic" => Self {
                enabled: true,
                preset: "comic".into(),
                duration_seconds: 3.5,
                card_bg_color: "#FFE600".into(),
                text_color: "#000000".into(),
                kicker_bg_color: "#FF0055".into(),
                kicker_text_color: "#FFFFFF".into(),
                y_position_ratio: 0.15,
            },
            "sticker" => Self {
                enabled: true,
                preset: "sticker".into(),
                duration_seconds: 3.2,
                card_bg_color: "#1E1B4B".into(),
                text_color: "#F8FAFC".into(),
                kicker_bg_color: "#00F5D4".into(),
                kicker_text_color: "#0F172A".into(),
                y_position_ratio: 0.14,
            },
            _ => Self {
                enabled: true,
                preset: "punchline".into(),
                duration_seconds: 3.2,
                card_bg_color: "#FFFFFF".into(),
                text_color: "#111827".into(),
                kicker_bg_color: "#F43F5E".into(),
                kicker_text_color: "#FFFFFF".into(),
                y_position_ratio: 0.14,
            },
        }
    }
}
