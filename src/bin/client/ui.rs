//! Declarative UI rendering for protocol `UiNode` trees.
//!
//! Contract: `contracts/target/modules/client/ui.md`.

use std::collections::BTreeMap;

use macroquad::prelude::*;
use wesnoth_engine::engine::protocol::{Action, UiKind, UiNode};
use wesnoth_engine::value::Value;

use crate::widgets::{Input, Ui};

const GAP: f32 = 8.0;

pub struct Assets {
    textures: BTreeMap<String, Texture2D>,
}

impl Assets {
    pub fn from_memory(
        registry: &Value,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, String> {
        let mut textures = BTreeMap::new();
        for (id, media_type, _) in image_assets(registry)? {
            let bytes = resources
                .get(id)
                .ok_or_else(|| format!("UI asset was not loaded: {id}"))?;
            let format = image_format(media_type);
            let texture = Texture2D::from_file_with_format(bytes, format);
            if textures.insert(id.to_owned(), texture).is_some() {
                return Err(format!("duplicate UI asset id: {id}"));
            }
        }
        Ok(Self { textures })
    }

    fn get(&self, id: &str) -> Result<&Texture2D, String> {
        self.textures
            .get(id)
            .ok_or_else(|| format!("UI references unavailable image asset: {id}"))
    }
}

fn image_assets(registry: &Value) -> Result<Vec<(&str, &str, &str)>, String> {
    if registry.get("schema").and_then(Value::as_str) != Some("assets") {
        return Err("asset block has an unsupported schema".into());
    }
    let Value::List(items) = registry.get("items").ok_or("asset block has no items")? else {
        return Err("asset items must be a list".into());
    };
    items
        .iter()
        .filter_map(|item| {
            let media_type = item.get("media_type").and_then(Value::as_str)?;
            media_type.starts_with("image/").then_some((item, media_type))
        })
        .map(|(item, media_type)| {
            Ok((
                item.get("id")
                    .and_then(Value::as_str)
                    .ok_or("asset id must be a string")?,
                media_type,
                item.get("path")
                    .and_then(Value::as_str)
                    .ok_or("asset path must be a string")?,
            ))
        })
        .collect()
}

fn image_format(media_type: &str) -> Option<ImageFormat> {
    match media_type {
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        _ => None,
    }
}

pub fn draw(
    ui: &Ui,
    input: &Input,
    font: &Font,
    assets: &Assets,
    node: &UiNode,
    rect: Rect,
    locally_enabled: bool,
) -> Result<Option<Action>, String> {
    match node.kind {
        UiKind::Row => draw_children(ui, input, font, assets, node, rect, locally_enabled, true),
        UiKind::Column | UiKind::List => {
            draw_children(ui, input, font, assets, node, rect, locally_enabled, false)
        }
        UiKind::Panel => {
            ui.panel(rect);
            draw_children(
                ui,
                input,
                font,
                assets,
                node,
                inset(rect, GAP),
                locally_enabled,
                false,
            )
        }
        UiKind::Text | UiKind::Tooltip => {
            ui.wrapped_label(
                font,
                node.text.as_deref().unwrap_or(""),
                rect,
                22.0,
                if node.enabled && locally_enabled {
                    WHITE
                } else {
                    GRAY
                },
            );
            Ok(None)
        }
        UiKind::Button => Ok((ui.button(
            input,
            font,
            rect,
            node.text.as_deref().unwrap_or(""),
            node.enabled && locally_enabled,
        ))
        .then(|| node.action.clone())
        .flatten()),
        UiKind::Image => {
            let texture = assets.get(node.asset.as_deref().unwrap_or(""))?;
            let scale = (rect.w / texture.width()).min(rect.h / texture.height());
            let image = Rect::new(
                rect.x + (rect.w - texture.width() * scale) / 2.0,
                rect.y + (rect.h - texture.height() * scale) / 2.0,
                texture.width() * scale,
                texture.height() * scale,
            );
            ui.image(texture, image);
            if rect.contains(input.position)
                && let Some(tooltip) = node
                    .children
                    .iter()
                    .find(|child| child.kind == UiKind::Tooltip)
            {
                let tooltip_rect = Rect::new(rect.x, rect.y + rect.h - 52.0, rect.w, 52.0);
                ui.shade(tooltip_rect, Color::from_rgba(18, 24, 29, 235));
                draw(
                    ui,
                    input,
                    font,
                    assets,
                    tooltip,
                    inset(tooltip_rect, 6.0),
                    locally_enabled,
                )?;
            }
            Ok(None)
        }
        UiKind::Progress => Err(format!(
            "UI node {} uses an unsupported renderer kind: {:?}",
            node.id, node.kind
        )),
    }
}

fn draw_children(
    ui: &Ui,
    input: &Input,
    font: &Font,
    assets: &Assets,
    node: &UiNode,
    rect: Rect,
    locally_enabled: bool,
    horizontal: bool,
) -> Result<Option<Action>, String> {
    if node.children.is_empty() {
        return Ok(None);
    }
    let count = node.children.len() as f32;
    let total_grow: f32 = node.children.iter().map(|child| child.grow as f32).sum();
    let extent = if horizontal { rect.w } else { rect.h };
    let available = extent - GAP * (count - 1.0);
    if !available.is_finite() || available <= 0.0 {
        return Err(format!("UI node {} has no layout space", node.id));
    }
    let mut offset = 0.0;
    for child in &node.children {
        let item_extent = available * child.grow as f32 / total_grow;
        let child_rect = if horizontal {
            Rect::new(rect.x + offset, rect.y, item_extent, rect.h)
        } else {
            Rect::new(rect.x, rect.y + offset, rect.w, item_extent)
        };
        if let Some(action) = draw(
            ui,
            input,
            font,
            assets,
            child,
            child_rect,
            locally_enabled && node.enabled,
        )? {
            return Ok(Some(action));
        }
        offset += item_extent + GAP;
    }
    Ok(None)
}

fn inset(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.x + amount,
        rect.y + amount,
        (rect.w - amount * 2.0).max(0.0),
        (rect.h - amount * 2.0).max(0.0),
    )
}
