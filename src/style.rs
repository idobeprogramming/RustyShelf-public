use eframe::egui::{self, Color32};
use std::{io::Read, path::Path};

pub const EXAMPLE: &str = include_str!("../examples/rustyshelf.css");
const MAX_BYTES: u64 = 65_536;

#[derive(Clone, Debug, Default)]
pub struct CustomStyle {
    pub background: Option<Color32>,
    pub surface: Option<Color32>,
    pub card: Option<Color32>,
    pub accent: Option<Color32>,
    pub text: Option<Color32>,
    pub muted: Option<Color32>,
    pub radius: Option<u8>,
    pub gap: Option<f32>,
}

impl CustomStyle {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        let mut text = String::new();
        file.take(MAX_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|error| error.to_string())?;
        if text.len() as u64 > MAX_BYTES {
            return Err("Le fichier CSS dépasse 64 Ko.".into());
        }
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut clean = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("/*") {
            clean.push_str(&rest[..start]);
            let end = rest[start + 2..]
                .find("*/")
                .ok_or("Commentaire CSS non fermé.")?;
            rest = &rest[start + 2 + end + 2..];
        }
        clean.push_str(rest);
        let body = clean
            .trim()
            .strip_prefix(":root")
            .map(str::trim)
            .and_then(|text| text.strip_prefix('{'))
            .and_then(|text| text.strip_suffix('}'))
            .ok_or("Utilise un seul bloc :root { --variable: valeur; }. Voir le modèle CSS.")?;
        let mut style = Self::default();
        for declaration in body
            .split(';')
            .map(str::trim)
            .filter(|text| !text.is_empty())
        {
            let (key, value) = declaration
                .split_once(':')
                .ok_or("Déclaration CSS invalide : ':' manquant.")?;
            let value = value.trim();
            match key.trim() {
                "--background" => style.background = Some(parse_color(value)?),
                "--surface" => style.surface = Some(parse_color(value)?),
                "--card" => style.card = Some(parse_color(value)?),
                "--accent" => style.accent = Some(parse_color(value)?),
                "--text" => style.text = Some(parse_color(value)?),
                "--muted" => style.muted = Some(parse_color(value)?),
                "--radius" => style.radius = Some(parse_pixels(value, 0.0, 24.0)? as u8),
                "--gap" => style.gap = Some(parse_pixels(value, 4.0, 24.0)?),
                key => return Err(format!("Variable non prise en charge : {key}")),
            }
        }
        Ok(style)
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let visuals = &mut style.visuals;
        if let Some(color) = self.background {
            visuals.panel_fill = color;
            visuals.extreme_bg_color = color;
        }
        if let Some(color) = self.surface {
            visuals.window_fill = color;
            visuals.widgets.noninteractive.bg_fill = color;
            visuals.widgets.inactive.bg_fill = color;
            visuals.widgets.inactive.weak_bg_fill = color;
            visuals.widgets.hovered.bg_fill = color.gamma_multiply(1.2);
            visuals.widgets.open.bg_fill = color;
        }
        if let Some(color) = self.accent {
            visuals.selection.bg_fill = color;
            visuals.widgets.active.bg_fill = color;
            visuals.widgets.active.fg_stroke.color = text_on(color);
            visuals.selection.stroke.color = text_on(color);
        }
        if let Some(color) = self.text {
            visuals.override_text_color = Some(color);
        }
        if let Some(radius) = self.radius {
            let radius = egui::CornerRadius::same(radius);
            visuals.window_corner_radius = radius;
            visuals.menu_corner_radius = radius;
            visuals.widgets.inactive.corner_radius = radius;
            visuals.widgets.hovered.corner_radius = radius;
            visuals.widgets.active.corner_radius = radius;
        }
        if let Some(gap) = self.gap {
            style.spacing.item_spacing = egui::vec2(gap, gap);
        }
        ctx.set_style(style);
    }
}

pub fn text_on(color: Color32) -> Color32 {
    if u32::from(color.r()) * 299 + u32::from(color.g()) * 587 + u32::from(color.b()) * 114
        > 150_000
    {
        Color32::from_rgb(20, 23, 29)
    } else {
        Color32::WHITE
    }
}

fn parse_color(value: &str) -> Result<Color32, String> {
    let hex = value
        .strip_prefix('#')
        .ok_or("Couleur attendue : #RGB, #RRGGBB ou #RRGGBBAA.")?;
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) || !matches!(hex.len(), 3 | 6 | 8) {
        return Err(format!("Couleur invalide : {value}"));
    }
    let hex = if hex.len() == 3 {
        hex.chars()
            .flat_map(|character| [character, character])
            .collect()
    } else {
        hex.to_owned()
    };
    let channel = |index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap();
    Ok(Color32::from_rgba_unmultiplied(
        channel(0),
        channel(2),
        channel(4),
        if hex.len() == 8 { channel(6) } else { 255 },
    ))
}

fn parse_pixels(value: &str, min: f32, max: f32) -> Result<f32, String> {
    let number = value
        .strip_suffix("px")
        .ok_or("Dimension attendue en px.")?
        .trim()
        .parse::<f32>()
        .map_err(|_| "Dimension CSS invalide.")?;
    if !number.is_finite() || !(min..=max).contains(&number) {
        return Err(format!("Dimension attendue entre {min} et {max} px."));
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_example_and_supports_css_comments_and_hex_colors() {
        assert!(CustomStyle::parse(EXAMPLE).is_ok());
        let style = CustomStyle::parse(
            "/* local */ :root { --accent: #abc; --text: #112233ff; --radius: 0px; --gap: 8px; }",
        )
        .unwrap();
        assert_eq!(style.accent, Some(Color32::from_rgb(170, 187, 204)));
        assert_eq!(style.text, Some(Color32::from_rgb(17, 34, 51)));
        assert_eq!(style.radius, Some(0));
    }

    #[test]
    fn rejects_unsupported_css_and_invalid_values() {
        for css in [
            "@import url(https://example.com);",
            ":root {--accent: url(image.png);}",
            ":root {--gap: NaNpx;}",
            ":root {--radius: 250px;}",
            ":root {--text: #zzzzzz;}",
            ":root {--font: Arial;}",
            "/* unclosed",
            ":root {--accent: #fff;} body {}",
        ] {
            assert!(CustomStyle::parse(css).is_err(), "{css}");
        }
    }

    #[test]
    fn rejects_oversized_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.css");
        std::fs::write(&path, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(CustomStyle::load(&path).unwrap_err().contains("64 Ko"));
    }
}
