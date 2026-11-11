use eframe::egui::{self, Color32, Pos2, Rect};

/// Native backdrop blur is configured only when the selected theme changes.
pub fn set_native(frame: &eframe::Frame, enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        let result = if enabled {
            window_vibrancy::apply_acrylic(frame, Some((20, 26, 38, 120)))
        } else {
            window_vibrancy::clear_acrylic(frame)
        };
        result.map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        // Linux compositors own backdrop blur. Alpha still reaches the window.
        let _ = (frame, enabled);
        Ok(())
    }
}

pub fn surface(painter: &egui::Painter, rect: Rect, radius: u8, fill: Color32) {
    painter.add(
        egui::epaint::Shadow {
            offset: [0, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(38),
        }
        .as_shape(rect, egui::CornerRadius::same(radius)),
    );
    painter.rect_filled(rect, radius, fill);
    painter.add(egui::Shape::mesh(reflection_mesh(rect, f32::from(radius))));
    painter.rect_stroke(
        rect.shrink(0.5),
        radius,
        egui::Stroke::new(1.0_f32, Color32::from_white_alpha(54)),
        egui::StrokeKind::Inside,
    );
}

// Rounded geometry keeps the highlight inside the glass instead of painting
// a rectangular gradient across its corners. No animation or repaint loop.
fn reflection_mesh(rect: Rect, radius: f32) -> egui::Mesh {
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let mut mesh = egui::Mesh::default();
    let mut vertex = |pos: Pos2| {
        let fraction = ((pos.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        mesh.colored_vertex(
            pos,
            Color32::from_white_alpha((34.0 * (1.0 - fraction) + 3.0) as u8),
        );
    };
    vertex(rect.center());
    let corners = [
        (
            egui::pos2(rect.right() - radius, rect.top() + radius),
            -90.0_f32,
        ),
        (
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            90.0,
        ),
        (egui::pos2(rect.left() + radius, rect.top() + radius), 180.0),
    ];
    for (center, start) in corners {
        for step in 0..=8 {
            let angle = (start + step as f32 * 90.0 / 8.0).to_radians();
            vertex(center + egui::vec2(angle.cos(), angle.sin()) * radius);
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for index in 1..=count {
        mesh.add_triangle(0, index, index % count + 1);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_reflections_stay_inside_small_and_large_cards() {
        for size in [egui::vec2(12.0, 8.0), egui::vec2(220.0, 196.0)] {
            let rect = Rect::from_min_size(egui::pos2(50.0, 70.0), size);
            let mesh = reflection_mesh(rect, 20.0);
            assert!(mesh.is_valid());
            assert!(mesh
                .vertices
                .iter()
                .all(|vertex| rect.expand(0.001).contains(vertex.pos)));
            assert!(mesh.vertices.iter().all(|vertex| vertex.color.a() < 40));
            let top = mesh
                .vertices
                .iter()
                .min_by(|a, b| a.pos.y.total_cmp(&b.pos.y))
                .unwrap();
            let bottom = mesh
                .vertices
                .iter()
                .max_by(|a, b| a.pos.y.total_cmp(&b.pos.y))
                .unwrap();
            assert!(top.color.a() > bottom.color.a());
        }
    }
}
