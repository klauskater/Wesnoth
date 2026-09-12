use macroquad::prelude::*;

pub const DESIGN_WIDTH: f32 = 1280.0;
pub const DESIGN_HEIGHT: f32 = 720.0;

pub struct Input {
    pub position: Vec2,
    pub pressed: bool,
}

pub struct Ui {
    scale: f32,
    offset: Vec2,
}

impl Ui {
    pub fn anchored(scale: f32, offset: Vec2) -> Self {
        Self { scale, offset }
    }

    pub fn image(&self, texture: &Texture2D, rect: Rect) {
        let rect = self.rect(rect);
        draw_texture_ex(
            texture,
            rect.x,
            rect.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(rect.w, rect.h)),
                ..Default::default()
            },
        );
    }

    pub fn new() -> Self {
        let scale = (screen_width() / DESIGN_WIDTH)
            .min(screen_height() / DESIGN_HEIGHT)
            .max(0.1);
        Self {
            scale,
            offset: vec2(
                (screen_width() - DESIGN_WIDTH * scale) / 2.0,
                (screen_height() - DESIGN_HEIGHT * scale) / 2.0,
            ),
        }
    }

    fn point(&self, point: Vec2) -> Vec2 {
        (point - self.offset) / self.scale
    }

    fn rect(&self, rect: Rect) -> Rect {
        Rect::new(
            self.offset.x + rect.x * self.scale,
            self.offset.y + rect.y * self.scale,
            rect.w * self.scale,
            rect.h * self.scale,
        )
    }

    pub fn input(&self) -> Input {
        let touch = touches()
            .into_iter()
            .find(|touch| touch.phase == TouchPhase::Started);
        let (position, pressed) = touch.map_or_else(
            || {
                (
                    Vec2::from(mouse_position()),
                    is_mouse_button_pressed(MouseButton::Left),
                )
            },
            |touch| (touch.position, true),
        );
        Input {
            position: self.point(position),
            pressed,
        }
    }

    pub fn button(
        &self,
        input: &Input,
        font: &Font,
        rect: Rect,
        text: &str,
        enabled: bool,
    ) -> bool {
        let hovered = enabled && rect.contains(input.position);
        self.shade(
            rect,
            if !enabled {
                Color::from_rgba(47, 51, 56, 235)
            } else if hovered {
                Color::from_rgba(93, 119, 145, 245)
            } else {
                Color::from_rgba(52, 76, 99, 235)
            },
        );
        self.outline(rect, 2.0, if enabled { GRAY } else { DARKGRAY });
        self.centered_label(font, text, rect, 25.0, if enabled { WHITE } else { GRAY });
        enabled && hovered && input.pressed
    }

    pub fn panel(&self, rect: Rect) {
        self.shade(rect, Color::from_rgba(18, 24, 29, 235));
        self.outline(rect, 3.0, GOLD);
    }

    pub fn shade(&self, rect: Rect, color: Color) {
        let rect = self.rect(rect);
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, color);
    }

    pub fn outline(&self, rect: Rect, thickness: f32, color: Color) {
        let rect = self.rect(rect);
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            thickness * self.scale,
            color,
        );
    }

    pub fn label(&self, font: &Font, value: &str, x: f32, y: f32, size: f32, color: Color) {
        draw_text_ex(
            value,
            self.offset.x + x * self.scale,
            self.offset.y + y * self.scale,
            TextParams {
                font: Some(font),
                font_size: (size * self.scale).round().max(1.0) as u16,
                color,
                ..Default::default()
            },
        );
    }

    pub fn wrapped_label(&self, font: &Font, value: &str, rect: Rect, size: f32, color: Color) {
        let mut line = String::new();
        let mut y = rect.y + size;
        for word in value.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty()
                && measure_text(&candidate, Some(font), size as u16, 1.0).width > rect.w
            {
                self.label(font, &line, rect.x, y, size, color);
                line = word.to_owned();
                y += size * 1.35;
            } else {
                line = candidate;
            }
            if y > rect.y + rect.h {
                break;
            }
        }
        if !line.is_empty() && y <= rect.y + rect.h {
            self.label(font, &line, rect.x, y, size, color);
        }
    }

    fn centered_label(&self, font: &Font, value: &str, rect: Rect, size: f32, color: Color) {
        let dimensions = measure_text(value, Some(font), size as u16, 1.0);
        self.label(
            font,
            value,
            rect.x + (rect.w - dimensions.width) / 2.0,
            rect.y + (rect.h + dimensions.height) / 2.0,
            size,
            color,
        );
    }
}

pub fn draw_background(texture: &Texture2D) {
    clear_background(Color::from_rgba(12, 17, 22, 255));
    let scale = (screen_width() / texture.width()).max(screen_height() / texture.height());
    let size = vec2(texture.width() * scale, texture.height() * scale);
    draw_texture_ex(
        texture,
        (screen_width() - size.x) / 2.0,
        (screen_height() - size.y) / 2.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(size),
            ..Default::default()
        },
    );
    draw_rectangle(
        0.0,
        0.0,
        screen_width(),
        screen_height(),
        Color::from_rgba(6, 10, 14, 80),
    );
}
