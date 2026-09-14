//! Recoverable client-side presentation cache.
//!
//! Contract: `contracts/target/modules/client/view.md`.

use std::collections::BTreeMap;

use wesnoth_engine::{
    engine::protocol::{self, Effect, Message, SceneItem, UiNode, ViewSnapshot, ViewUpdate},
    value::Value,
};

#[derive(Debug, PartialEq)]
pub enum Apply {
    Applied(Vec<Effect>),
    NeedSnapshot,
}

#[derive(Default)]
pub struct View {
    world_revision: u64,
    view_revision: u64,
    blocks: BTreeMap<String, Value>,
}

impl View {
    pub fn apply_snapshot(&mut self, snapshot: ViewSnapshot) -> Result<(), String> {
        protocol::validate(&Message::ViewSnapshot(snapshot.clone()))
            .map_err(|error| error.message)?;
        let blocks = snapshot
            .blocks
            .into_iter()
            .map(|block| (block.id, block.content))
            .collect();
        self.world_revision = snapshot.world_revision;
        self.view_revision = snapshot.view_revision;
        self.blocks = blocks;
        Ok(())
    }

    pub fn apply_update(&mut self, update: ViewUpdate) -> Result<Apply, String> {
        protocol::validate(&Message::ViewUpdate(update.clone())).map_err(|error| error.message)?;
        if update.base_view_revision != self.view_revision {
            return Ok(Apply::NeedSnapshot);
        }
        let mut blocks = self.blocks.clone();
        for id in update.remove_ids {
            blocks.remove(&id);
        }
        for block in update.replace_blocks {
            blocks.insert(block.id, block.content);
        }
        self.world_revision = update.world_revision;
        self.view_revision = update.view_revision;
        self.blocks = blocks;
        Ok(Apply::Applied(update.effects))
    }

    pub fn block(&self, id: &str) -> Option<&Value> {
        self.blocks.get(id)
    }

    pub fn ui_block(&self, block: &str) -> Result<Option<UiNode>, String> {
        self.block(block)
            .map(protocol::ui_block)
            .transpose()
            .map_err(|error| error.message)
    }

    pub fn scene_block(&self, block: &str) -> Result<Option<Vec<SceneItem>>, String> {
        self.block(block)
            .map(protocol::scene_block)
            .transpose()
            .map_err(|error| error.message)
    }

    #[allow(dead_code)] // Public contract entry; generic UI nodes are introduced separately.
    pub fn lookup(&self, block: &str, node: &str) -> Option<&Value> {
        find_id(self.block(block)?, node)
    }
}

#[allow(dead_code)]
fn find_id<'a>(value: &'a Value, wanted: &str) -> Option<&'a Value> {
    match value {
        Value::Map(fields) => {
            if fields.get("id").and_then(Value::as_str) == Some(wanted) {
                return Some(value);
            }
            fields.values().find_map(|value| find_id(value, wanted))
        }
        Value::List(values) => values.iter().find_map(|value| find_id(value, wanted)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wesnoth_engine::engine::protocol::ViewBlock;

    #[test]
    fn stale_update_is_atomic_and_snapshot_recovers() {
        let mut view = View::default();
        view.apply_snapshot(ViewSnapshot {
            world_revision: 1,
            view_revision: 2,
            blocks: vec![ViewBlock {
                id: "status".into(),
                content: Value::Integer(1),
            }],
        })
        .unwrap();
        let stale = ViewUpdate {
            world_revision: 2,
            base_view_revision: 1,
            view_revision: 2,
            replace_blocks: vec![ViewBlock {
                id: "status".into(),
                content: Value::Integer(2),
            }],
            remove_ids: Vec::new(),
            effects: Vec::new(),
        };
        assert_eq!(view.apply_update(stale).unwrap(), Apply::NeedSnapshot);
        assert_eq!(view.block("status"), Some(&Value::Integer(1)));
    }
}
