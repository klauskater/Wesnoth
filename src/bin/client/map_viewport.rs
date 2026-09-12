use macroquad::prelude::*;

const DRAG_SLOP: f32 = 6.0;
const MAX_ZOOM_FACTOR: f32 = 8.0;
const ZOOM_STEP: f32 = 1.25;
const ZOOM_SPEED: f32 = 12.0;

pub struct MapViewport {
    center: Vec2,
    zoom: f32,
    target_zoom: f32,
    zoom_anchor: Option<(Vec2, Vec2)>,
    min: Vec2,
    max: Vec2,
    drag: Option<(Vec2, Vec2)>,
    dragged: bool,
    reserved_width: f32,
}

impl MapViewport {
    pub fn new(min: Vec2, max: Vec2, screen: Vec2) -> Self {
        let mut viewport = Self {
            center: (min + max) / 2.0,
            zoom: 1.0,
            target_zoom: 1.0,
            zoom_anchor: None,
            min,
            max,
            drag: None,
            dragged: false,
            reserved_width: 0.0,
        };
        viewport.fit(screen);
        viewport
    }

    /// Updates the camera and returns a world-space click, never a drag release.
    pub fn update(&mut self, screen: Vec2) -> Option<Vec2> {
        let screen = self.map_area(screen);
        if is_key_pressed(KeyCode::Home) {
            self.fit(screen);
        }

        let cursor = Vec2::from(mouse_position());
        let inside = Rect::new(0.0, 0.0, screen.x, screen.y).contains(cursor);
        let wheel = mouse_wheel().1;
        if inside && wheel != 0.0 {
            let world_under_cursor = self.unproject(cursor, screen);
            let min_zoom = fit_zoom(self.min, self.max, screen);
            self.target_zoom = (self.target_zoom * ZOOM_STEP.powf(wheel_steps(wheel)))
                .clamp(min_zoom, min_zoom * MAX_ZOOM_FACTOR);
            self.zoom_anchor = Some((cursor, world_under_cursor));
        }

        if inside && is_mouse_button_pressed(MouseButton::Left) {
            self.target_zoom = self.zoom;
            self.zoom_anchor = None;
            self.drag = Some((cursor, self.center));
            self.dragged = false;
        }
        if let Some((start, center)) = self.drag
            && is_mouse_button_down(MouseButton::Left)
        {
            let delta = cursor - start;
            self.dragged |= delta.length() >= DRAG_SLOP;
            if self.dragged {
                self.center = center - delta / self.zoom;
            }
        }

        let click = if is_mouse_button_released(MouseButton::Left) && self.drag.is_some() {
            (!self.dragged && inside).then(|| self.unproject(cursor, screen))
        } else {
            None
        };
        if is_mouse_button_released(MouseButton::Left) {
            self.drag = None;
        }

        let min_zoom = fit_zoom(self.min, self.max, screen);
        self.target_zoom = self.target_zoom.clamp(min_zoom, min_zoom * MAX_ZOOM_FACTOR);
        if let Some((anchor, world)) = self.zoom_anchor {
            self.zoom = advance_zoom(self.zoom, self.target_zoom, get_frame_time());
            self.center = world - (anchor - screen / 2.0) / self.zoom;
            if (self.zoom / self.target_zoom - 1.0).abs() < 0.001 {
                self.zoom = self.target_zoom;
                self.zoom_anchor = None;
            }
        } else {
            self.zoom = self.zoom.clamp(min_zoom, min_zoom * MAX_ZOOM_FACTOR);
        }
        self.clamp_center(screen);
        click
    }

    pub fn project(&self, world: Vec2, screen: Vec2) -> Vec2 {
        self.map_area(screen) / 2.0 + (world - self.center) * self.zoom
    }

    pub fn reserve_panel(&mut self, width: f32) {
        self.reserved_width = width;
    }

    fn map_area(&self, screen: Vec2) -> Vec2 {
        vec2((screen.x - self.reserved_width).max(1.0), screen.y)
    }

    pub fn focus(&mut self, world: Vec2) {
        self.center = world;
        self.zoom_anchor = None;
    }

    pub fn cancel_gesture(&mut self) {
        self.drag = None;
        self.zoom_anchor = None;
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    fn unproject(&self, screen_point: Vec2, screen: Vec2) -> Vec2 {
        self.center + (screen_point - screen / 2.0) / self.zoom
    }

    fn fit(&mut self, screen: Vec2) {
        self.center = (self.min + self.max) / 2.0;
        self.zoom = fit_zoom(self.min, self.max, screen);
        self.target_zoom = self.zoom;
        self.zoom_anchor = None;
    }

    fn clamp_center(&mut self, screen: Vec2) {
        let visible_half = screen / (2.0 * self.zoom);
        self.center.x = clamp_axis(self.center.x, self.min.x, self.max.x, visible_half.x);
        self.center.y = clamp_axis(self.center.y, self.min.y, self.max.y, visible_half.y);
    }
}

fn wheel_steps(delta: f32) -> f32 {
    if delta.abs() > 10.0 {
        delta.signum()
    } else {
        delta.clamp(-1.0, 1.0)
    }
}

fn advance_zoom(current: f32, target: f32, seconds: f32) -> f32 {
    let progress = 1.0 - (-ZOOM_SPEED * seconds).exp();
    current * (target / current).powf(progress)
}

fn fit_zoom(min: Vec2, max: Vec2, screen: Vec2) -> f32 {
    let size = max - min;
    ((screen.x - 32.0) / size.x)
        .min((screen.y - 32.0) / size.y)
        .max(0.1)
}

fn clamp_axis(value: f32, min: f32, max: f32, visible_half: f32) -> f32 {
    if max - min <= visible_half * 2.0 {
        (min + max) / 2.0
    } else {
        value.clamp(min + visible_half, max - visible_half)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitted_view_centers_the_world() {
        let viewport = MapViewport::new(vec2(-2.0, -1.0), vec2(8.0, 5.0), vec2(1000.0, 600.0));
        assert_eq!(
            viewport.project(vec2(3.0, 2.0), vec2(1000.0, 600.0)),
            vec2(500.0, 300.0)
        );
    }

    #[test]
    fn normalizes_windows_wheel_delta_and_eases_zoom() {
        assert_eq!(wheel_steps(120.0), 1.0);
        let zoom = advance_zoom(10.0, 20.0, 1.0 / 60.0);
        assert!(zoom > 10.0 && zoom < 20.0);
    }

    #[test]
    fn panel_reserves_space_for_projection_and_hit_testing() {
        let screen = vec2(1280.0, 720.0);
        let mut viewport = MapViewport::new(vec2(-2.0, -1.0), vec2(8.0, 5.0), screen);
        viewport.reserve_panel(280.0);
        let area = viewport.map_area(screen);
        assert_eq!(area, vec2(1000.0, 720.0));
        let world = vec2(3.0, 2.0);
        let projected = viewport.project(world, screen);
        assert_eq!(projected, vec2(500.0, 360.0));
        assert_eq!(viewport.unproject(projected, area), world);
        assert!(!Rect::new(0.0, 0.0, area.x, area.y).contains(vec2(1100.0, 300.0)));
    }
}
