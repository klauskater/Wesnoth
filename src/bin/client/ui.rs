//! Declarative UI rendering for protocol `UiNode` trees.
//!
//! Contract: `contracts/target/modules/client/ui.md`.

use macroquad::prelude::*;
use wesnoth_engine::engine::protocol::{Action, UiKind, UiNode};

use crate::widgets::{Input, Ui};

const GAP: f32 = 8.0;

pub fn draw(
    ui: &Ui,
    input: &Input,
    font: &Font,
    node: &UiNode,
    rect: Rect,
    locally_enabled: bool,
) -> Result<Option<Action>, String> {
    match node.kind {
        UiKind::Row => draw_children(ui, input, font, node, rect, locally_enabled, true),
        UiKind::Column | UiKind::List => {
            draw_children(ui, input, font, node, rect, locally_enabled, false)
        }
        UiKind::Panel => {
            ui.panel(rect);
            draw_children(
                ui,
                input,
                font,
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
        UiKind::Image | UiKind::Progress => Err(format!(
            "UI node {} uses an unsupported renderer kind: {:?}",
            node.id, node.kind
        )),
    }
}

fn draw_children(
    ui: &Ui,
    input: &Input,
    font: &Font,
    node: &UiNode,
    rect: Rect,
    locally_enabled: bool,
    horizontal: bool,
) -> Result<Option<Action>, String> {
    if node.children.is_empty() {
        return Ok(None);
    }
    let count = node.children.len() as f32;
    let extent = if horizontal { rect.w } else { rect.h };
    let item_extent = (extent - GAP * (count - 1.0)) / count;
    if !item_extent.is_finite() || item_extent <= 0.0 {
        return Err(format!("UI node {} has no layout space", node.id));
    }
    for (index, child) in node.children.iter().enumerate() {
        let offset = index as f32 * (item_extent + GAP);
        let child_rect = if horizontal {
            Rect::new(rect.x + offset, rect.y, item_extent, rect.h)
        } else {
            Rect::new(rect.x, rect.y + offset, rect.w, item_extent)
        };
        if let Some(action) = draw(
            ui,
            input,
            font,
            child,
            child_rect,
            locally_enabled && node.enabled,
        )? {
            return Ok(Some(action));
        }
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
