//! Game-agnostic engine/client message schema and validation.
//!
//! Actions and payloads are opaque Values. Contract:
//! `contracts/target/modules/engine/protocol.md`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{Position, value::Value};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

impl Error {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub command_id: String,
    pub expected_world_revision: u64,
    pub action: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    Activate,
    CellClick,
    Hover,
    Dismiss,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    pub interaction_id: String,
    pub expected_view_revision: u64,
    pub kind: InteractionKind,
    pub target: Value,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Committed,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResult {
    pub command_id: String,
    pub status: CommandStatus,
    pub world_revision: u64,
    pub error: Option<Error>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewBlock {
    pub id: String,
    pub content: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub id: String,
    pub content: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub replace_blocks: Vec<ViewBlock>,
    pub remove_ids: Vec<String>,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub action: String,
    pub payload: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiKind {
    Row,
    Column,
    Panel,
    Text,
    Image,
    Button,
    List,
    Progress,
    Tooltip,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiNode {
    pub id: String,
    pub kind: UiKind,
    pub text: Option<String>,
    pub asset: Option<String>,
    pub grow: f64,
    pub enabled: bool,
    pub action: Option<Action>,
    pub children: Vec<UiNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneItem {
    pub id: String,
    pub asset: String,
    pub frames: Vec<String>,
    pub frame_ms: u32,
    pub phase_ms: u32,
    pub anchor: Position,
    pub offset: [f64; 2],
    pub layer: String,
    pub order: i64,
    pub tint: [f64; 4],
    pub hit_id: Option<String>,
    pub clips: Vec<Position>,
    pub crop: Option<[u32; 4]>,
    pub masks: Vec<String>,
    pub opacity: u8,
}

pub fn ui_block(value: &Value) -> Result<UiNode, Error> {
    let Value::Map(block) = value else {
        return Err(invalid("UI block must be a record"));
    };
    if block.get("schema").and_then(Value::as_str) != Some("ui") {
        return Err(invalid("UI block must declare the ui schema"));
    }
    if block.contains_key("map_tint") {
        let tint = number_array::<4>(block.get("map_tint"), [1.0; 4], "UI block map_tint")?;
        if tint.iter().any(|channel| !(0.0..=1.0).contains(channel)) {
            return Err(invalid("UI block map_tint entries must be in 0..1"));
        }
    }
    if block
        .get("scene_dark")
        .is_some_and(|value| !matches!(value, Value::Bool(_)))
    {
        return Err(invalid("UI block scene_dark must be a boolean"));
    }
    let root = block
        .get("root")
        .ok_or_else(|| invalid("UI block has no root node"))?;
    let mut ids = BTreeSet::new();
    parse_ui_node(root, &mut ids)
}

pub fn scene_block(value: &Value) -> Result<Vec<SceneItem>, Error> {
    let Value::Map(block) = value else {
        return Err(invalid("scene block must be a record"));
    };
    if block.get("schema").and_then(Value::as_str) != Some("scene") {
        return Err(invalid("scene block must declare the scene schema"));
    }
    let Some(Value::List(items)) = block.get("items") else {
        return Err(invalid("scene block items must be a list"));
    };
    let mut ids = BTreeSet::new();
    items
        .iter()
        .map(|item| parse_scene_item(item, &mut ids))
        .collect()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionResult {
    pub view_context: Value,
    pub command: Option<Action>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSnapshot {
    pub world_revision: u64,
    pub view_revision: u64,
    pub blocks: Vec<ViewBlock>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewUpdate {
    pub world_revision: u64,
    pub base_view_revision: u64,
    pub view_revision: u64,
    pub replace_blocks: Vec<ViewBlock>,
    pub remove_ids: Vec<String>,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "body", rename_all = "snake_case")]
pub enum Message {
    Command(Command),
    Interaction(Interaction),
    CommandResult(CommandResult),
    ViewSnapshot(ViewSnapshot),
    ViewUpdate(ViewUpdate),
    Error(Error),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    protocol_version: u32,
    message: Message,
}

pub fn validate(message: &Message) -> Result<(), Error> {
    match message {
        Message::Command(command) => {
            id(&command.command_id, "command_id")?;
            id(&command.action, "action")?;
            value(&command.payload)
        }
        Message::Interaction(interaction) => {
            id(&interaction.interaction_id, "interaction_id")?;
            value(&interaction.target)?;
            value(&interaction.payload)
        }
        Message::CommandResult(result) => {
            id(&result.command_id, "command_id")?;
            match (&result.status, &result.error) {
                (CommandStatus::Committed, None) | (CommandStatus::Rejected, Some(_)) => Ok(()),
                _ => Err(invalid("command status and error disagree")),
            }?;
            if let Some(error) = &result.error {
                validate_error(error)?;
            }
            Ok(())
        }
        Message::ViewSnapshot(snapshot) => blocks(&snapshot.blocks),
        Message::ViewUpdate(update) => {
            if update.view_revision <= update.base_view_revision {
                return Err(invalid("view revision must advance"));
            }
            blocks(&update.replace_blocks)?;
            unique(
                update.remove_ids.iter().map(String::as_str),
                "removed block",
            )?;
            let replaced: BTreeSet<_> = update
                .replace_blocks
                .iter()
                .map(|block| block.id.as_str())
                .collect();
            if update
                .remove_ids
                .iter()
                .any(|id| replaced.contains(id.as_str()))
            {
                return Err(invalid("a block cannot be replaced and removed together"));
            }
            unique(
                update.effects.iter().map(|effect| effect.id.as_str()),
                "effect",
            )?;
            update
                .effects
                .iter()
                .try_for_each(|effect| value(&effect.content))
        }
        Message::Error(candidate) => validate_error(candidate),
    }
}

pub fn validate_assets(message: &Message, known: &BTreeSet<String>) -> Result<(), Error> {
    validate(message)?;
    match message {
        Message::ViewSnapshot(snapshot) => snapshot
            .blocks
            .iter()
            .try_for_each(|block| asset_references(&block.content, known)),
        Message::ViewUpdate(update) => update
            .replace_blocks
            .iter()
            .map(|block| &block.content)
            .chain(update.effects.iter().map(|effect| &effect.content))
            .try_for_each(|content| asset_references(content, known)),
        _ => Ok(()),
    }
}

pub fn encode(message: &Message) -> Result<Vec<u8>, Error> {
    validate(message)?;
    serde_json::to_vec(&Envelope {
        protocol_version: PROTOCOL_VERSION,
        message: message.clone(),
    })
    .map_err(|error| invalid(error.to_string()))
}

pub fn decode(bytes: &[u8]) -> Result<Message, Error> {
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
    if envelope.protocol_version != PROTOCOL_VERSION {
        return Err(Error::new(
            "incompatible_version",
            format!(
                "unsupported protocol version: {} (expected {PROTOCOL_VERSION})",
                envelope.protocol_version
            ),
        ));
    }
    validate(&envelope.message)?;
    Ok(envelope.message)
}

fn blocks(blocks: &[ViewBlock]) -> Result<(), Error> {
    unique(blocks.iter().map(|block| block.id.as_str()), "block")?;
    blocks.iter().try_for_each(|block| {
        value(&block.content)?;
        if block.content.get("schema").and_then(Value::as_str) == Some("ui") {
            ui_block(&block.content)?;
        }
        if block.content.get("schema").and_then(Value::as_str) == Some("scene") {
            scene_block(&block.content)?;
        }
        Ok(())
    })
}

fn parse_scene_item(candidate: &Value, ids: &mut BTreeSet<String>) -> Result<SceneItem, Error> {
    let Value::Map(fields) = candidate else {
        return Err(invalid("scene item must be a record"));
    };
    let item_id = string_field(fields, "id", "scene item id")?;
    if !ids.insert(item_id.to_owned()) {
        return Err(invalid(format!("duplicate scene item id: {item_id}")));
    }
    let asset = string_field(fields, "asset", "scene item asset")?.to_owned();
    let frames = string_list(fields.get("frames"), "scene item frames")?
        .unwrap_or_else(|| vec![asset.clone()]);
    if frames.is_empty() || frames.first() != Some(&asset) {
        return Err(invalid(
            "scene item frames must start with its primary asset",
        ));
    }
    let frame_ms = positive_u32(fields.get("frame_ms"), 1, "scene item frame_ms")?;
    let phase_ms = nonnegative_u32(fields.get("phase_ms"), 0, "scene item phase_ms")?;
    let anchor = position_value(
        fields
            .get("anchor")
            .ok_or_else(|| invalid("scene item has no anchor"))?,
        "scene item anchor",
    )?;
    let offset = number_array::<2>(fields.get("offset"), [0.0, 0.0], "scene item offset")?;
    let layer = string_field(fields, "layer", "scene item layer")?.to_owned();
    let order = optional_i64(fields.get("order"), 0, "scene item order")?;
    let tint = number_array::<4>(fields.get("tint"), [1.0, 1.0, 1.0, 1.0], "scene item tint")?;
    if tint
        .iter()
        .any(|component| !(0.0..=1.0).contains(component))
    {
        return Err(invalid("scene item tint components must be within 0..=1"));
    }
    let hit_id = optional_string(fields.get("hit_id"), "scene item hit_id")?;
    let clips = match fields.get("clips") {
        None => Vec::new(),
        Some(Value::List(clips)) => clips
            .iter()
            .map(|clip| position_value(clip, "scene item clip"))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(invalid("scene item clips must be a list")),
    };
    let crop = match fields.get("crop") {
        None => None,
        Some(value) => {
            let values = integer_array::<4>(value, "scene item crop")?;
            if values[2] == 0 || values[3] == 0 {
                return Err(invalid("scene item crop dimensions must be positive"));
            }
            Some(values)
        }
    };
    let masks = string_list(fields.get("masks"), "scene item masks")?.unwrap_or_default();
    let opacity = nonnegative_u32(fields.get("opacity"), 255, "scene item opacity")?;
    let opacity =
        u8::try_from(opacity).map_err(|_| invalid("scene item opacity must be within 0..=255"))?;
    Ok(SceneItem {
        id: item_id.to_owned(),
        asset,
        frames,
        frame_ms,
        phase_ms,
        anchor,
        offset,
        layer,
        order,
        tint,
        hit_id,
        clips,
        crop,
        masks,
        opacity,
    })
}

fn parse_ui_node(candidate: &Value, ids: &mut BTreeSet<String>) -> Result<UiNode, Error> {
    let Value::Map(fields) = candidate else {
        return Err(invalid("UI node must be a record"));
    };
    let node_id = string_field(fields, "id", "UI node id")?;
    if !ids.insert(node_id.to_owned()) {
        return Err(invalid(format!("duplicate UI node id: {node_id}")));
    }
    let kind = match string_field(fields, "kind", "UI node kind")? {
        "row" => UiKind::Row,
        "column" => UiKind::Column,
        "panel" => UiKind::Panel,
        "text" => UiKind::Text,
        "image" => UiKind::Image,
        "button" => UiKind::Button,
        "list" => UiKind::List,
        "progress" => UiKind::Progress,
        "tooltip" => UiKind::Tooltip,
        other => return Err(invalid(format!("unknown UI node kind: {other}"))),
    };
    let text = match fields.get("text") {
        None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => return Err(invalid("UI node text must be a string")),
    };
    let asset = match fields.get("asset") {
        None => None,
        Some(Value::String(asset)) if !asset.is_empty() => Some(asset.clone()),
        Some(Value::String(_)) => return Err(invalid("UI node asset must not be empty")),
        Some(_) => return Err(invalid("UI node asset must be a string")),
    };
    if kind == UiKind::Image && asset.is_none() {
        return Err(invalid("image UI node has no asset"));
    }
    let grow = match fields.get("grow") {
        None => 1.0,
        Some(value) => value
            .as_f64()
            .ok_or_else(|| invalid("UI node grow must be a number"))?,
    };
    if !grow.is_finite() || grow <= 0.0 {
        return Err(invalid("UI node grow must be finite and positive"));
    }
    let enabled = match fields.get("enabled") {
        None => true,
        Some(Value::Bool(enabled)) => *enabled,
        Some(_) => return Err(invalid("UI node enabled must be a boolean")),
    };
    let action = match fields.get("action") {
        None => None,
        Some(Value::Map(action)) => Some(Action {
            action: string_field(action, "action", "UI action")?.to_owned(),
            payload: action
                .get("payload")
                .cloned()
                .ok_or_else(|| invalid("UI action has no payload"))?,
        }),
        Some(_) => return Err(invalid("UI action must be a record")),
    };
    let children = match fields.get("children") {
        None => Vec::new(),
        Some(Value::List(children)) => children
            .iter()
            .map(|child| parse_ui_node(child, ids))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(invalid("UI node children must be a list")),
    };
    Ok(UiNode {
        id: node_id.to_owned(),
        kind,
        text,
        asset,
        grow,
        enabled,
        action,
        children,
    })
}

fn position_value(candidate: &Value, name: &str) -> Result<Position, Error> {
    let Value::Map(fields) = candidate else {
        return Err(invalid(format!("{name} must be a record")));
    };
    Ok(Position {
        x: required_i64(fields.get("x"), &format!("{name}.x"))?,
        y: required_i64(fields.get("y"), &format!("{name}.y"))?,
    })
}

fn required_i64(candidate: Option<&Value>, name: &str) -> Result<i64, Error> {
    match candidate {
        Some(Value::Integer(value)) => Ok(*value),
        _ => Err(invalid(format!("{name} must be an integer"))),
    }
}

fn optional_i64(candidate: Option<&Value>, default: i64, name: &str) -> Result<i64, Error> {
    candidate.map_or(Ok(default), |value| required_i64(Some(value), name))
}

fn positive_u32(candidate: Option<&Value>, default: u32, name: &str) -> Result<u32, Error> {
    let value = nonnegative_u32(candidate, default, name)?;
    if value == 0 {
        Err(invalid(format!("{name} must be positive")))
    } else {
        Ok(value)
    }
}

fn nonnegative_u32(candidate: Option<&Value>, default: u32, name: &str) -> Result<u32, Error> {
    let Some(candidate) = candidate else {
        return Ok(default);
    };
    let value = required_i64(Some(candidate), name)?;
    u32::try_from(value).map_err(|_| invalid(format!("{name} must be a nonnegative u32")))
}

fn optional_string(candidate: Option<&Value>, name: &str) -> Result<Option<String>, Error> {
    match candidate {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{name} must be a nonempty string"))),
    }
}

fn string_list(candidate: Option<&Value>, name: &str) -> Result<Option<Vec<String>>, Error> {
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let Value::List(values) = candidate else {
        return Err(invalid(format!("{name} must be a list")));
    };
    values
        .iter()
        .map(|value| match value {
            Value::String(value) if !value.trim().is_empty() => Ok(value.clone()),
            _ => Err(invalid(format!("{name} entries must be nonempty strings"))),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn number_array<const N: usize>(
    candidate: Option<&Value>,
    default: [f64; N],
    name: &str,
) -> Result<[f64; N], Error> {
    let Some(candidate) = candidate else {
        return Ok(default);
    };
    let Value::List(values) = candidate else {
        return Err(invalid(format!("{name} must be a list of {N} numbers")));
    };
    if values.len() != N {
        return Err(invalid(format!("{name} must contain {N} numbers")));
    }
    let values = values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(format!("{name} entries must be finite numbers")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values.try_into().expect("length checked"))
}

fn integer_array<const N: usize>(candidate: &Value, name: &str) -> Result<[u32; N], Error> {
    let Value::List(values) = candidate else {
        return Err(invalid(format!("{name} must be a list of {N} integers")));
    };
    if values.len() != N {
        return Err(invalid(format!("{name} must contain {N} integers")));
    }
    let values = values
        .iter()
        .map(|value| {
            let Value::Integer(value) = value else {
                return Err(invalid(format!("{name} entries must be integers")));
            };
            u32::try_from(*value)
                .map_err(|_| invalid(format!("{name} entries must be nonnegative u32")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values.try_into().expect("length checked"))
}

fn string_field<'a>(
    fields: &'a std::collections::BTreeMap<String, Value>,
    field: &str,
    name: &str,
) -> Result<&'a str, Error> {
    match fields.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value),
        _ => Err(invalid(format!("{name} must be a nonempty string"))),
    }
}

fn unique<'a>(ids: impl IntoIterator<Item = &'a str>, kind: &str) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for candidate in ids {
        id(candidate, kind)?;
        if !seen.insert(candidate) {
            return Err(invalid(format!("duplicate {kind} id: {candidate}")));
        }
    }
    Ok(())
}

fn id(value: &str, name: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        Err(invalid(format!("{name} must not be empty")))
    } else {
        Ok(())
    }
}

fn value(candidate: &Value) -> Result<(), Error> {
    match candidate {
        Value::Number(number) if !number.is_finite() => Err(invalid("number must be finite")),
        Value::List(values) => values.iter().try_for_each(value),
        Value::Map(values) => values.values().try_for_each(value),
        _ => Ok(()),
    }
}

fn asset_references(candidate: &Value, known: &BTreeSet<String>) -> Result<(), Error> {
    match candidate {
        Value::Map(fields) => {
            if let Some(asset) = fields.get("asset") {
                asset_reference(asset, known)?;
            }
            for key in ["assets", "frames", "masks"] {
                if let Some(Value::List(assets)) = fields.get(key) {
                    assets
                        .iter()
                        .try_for_each(|asset| asset_reference(asset, known))?;
                }
            }
            fields
                .values()
                .try_for_each(|value| asset_references(value, known))
        }
        Value::List(values) => values
            .iter()
            .try_for_each(|value| asset_references(value, known)),
        _ => Ok(()),
    }
}

fn asset_reference(candidate: &Value, known: &BTreeSet<String>) -> Result<(), Error> {
    let Value::String(asset) = candidate else {
        return Err(invalid("asset reference must be a string"));
    };
    id(asset, "asset reference")?;
    if known.contains(asset) {
        Ok(())
    } else {
        Err(Error::new(
            "resource_error",
            format!("unknown asset reference: {asset}"),
        ))
    }
}

fn validate_error(candidate: &Error) -> Result<(), Error> {
    id(&candidate.code, "error code")?;
    id(&candidate.message, "error message")?;
    if let Some(details) = &candidate.details {
        value(details)?;
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new("invalid_input", message)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn opaque_actions_round_trip_and_invalid_updates_are_rejected() {
        let message = Message::Command(Command {
            command_id: "c1".into(),
            expected_world_revision: 4,
            action: "another_game.custom_action".into(),
            payload: Value::Map(BTreeMap::from([("anything".into(), Value::Bool(true))])),
        });
        assert_eq!(decode(&encode(&message).unwrap()).unwrap(), message);

        let error = Message::Error(Error {
            code: "another_game.failure".into(),
            message: "failed".into(),
            details: Some(Value::String("opaque".into())),
        });
        assert_eq!(decode(&encode(&error).unwrap()).unwrap(), error);
        assert!(decode(br#"{"protocol_version":1,"message":{"type":"error","body":{"code":"bad","message":"bad","details":null,"extra":true}}}"#).is_err());

        let update = Message::ViewUpdate(ViewUpdate {
            world_revision: 4,
            base_view_revision: 2,
            view_revision: 3,
            replace_blocks: vec![ViewBlock {
                id: "map".into(),
                content: Value::Null,
            }],
            remove_ids: vec!["map".into()],
            effects: Vec::new(),
        });
        assert!(validate(&update).is_err());

        let assets = BTreeSet::from(["ui.banner".to_owned()]);
        let block = |asset: &str| {
            Message::ViewSnapshot(ViewSnapshot {
                world_revision: 0,
                view_revision: 1,
                blocks: vec![ViewBlock {
                    id: "ui".into(),
                    content: Value::Map(BTreeMap::from([(
                        "asset".into(),
                        Value::String(asset.into()),
                    )])),
                }],
            })
        };
        assert!(validate_assets(&block("ui.banner"), &assets).is_ok());
        assert_eq!(
            validate_assets(&block("ui.missing"), &assets)
                .unwrap_err()
                .code,
            "resource_error"
        );

        let root = Value::Map(BTreeMap::from([
            ("id".into(), Value::String("hud".into())),
            ("kind".into(), Value::String("column".into())),
            (
                "children".into(),
                Value::List(vec![
                    Value::Map(BTreeMap::from([
                        ("id".into(), Value::String("end_turn".into())),
                        ("kind".into(), Value::String("button".into())),
                        ("text".into(), Value::String("End turn".into())),
                        ("enabled".into(), Value::Bool(true)),
                        (
                            "action".into(),
                            Value::Map(BTreeMap::from([
                                ("action".into(), Value::String("end_turn".into())),
                                ("payload".into(), Value::Null),
                            ])),
                        ),
                    ])),
                    Value::Map(BTreeMap::from([
                        ("id".into(), Value::String("banner".into())),
                        ("kind".into(), Value::String("image".into())),
                        ("asset".into(), Value::String("ui.banner".into())),
                        ("grow".into(), Value::Integer(3)),
                    ])),
                ]),
            ),
        ]));
        let ui = Value::Map(BTreeMap::from([
            ("schema".into(), Value::String("ui".into())),
            ("root".into(), root),
        ]));
        assert_eq!(ui_block(&ui).unwrap().children[0].kind, UiKind::Button);
        let image = &ui_block(&ui).unwrap().children[1];
        assert_eq!(image.asset.as_deref(), Some("ui.banner"));
        assert_eq!(image.grow, 3.0);

        let scene_item = Value::Map(BTreeMap::from([
            ("id".into(), Value::String("tile:1".into())),
            ("asset".into(), Value::String("terrain.grass".into())),
            (
                "anchor".into(),
                Value::Map(BTreeMap::from([
                    ("x".into(), Value::Integer(1)),
                    ("y".into(), Value::Integer(2)),
                ])),
            ),
            ("layer".into(), Value::String("ground".into())),
        ]));
        let scene = |items| {
            Value::Map(BTreeMap::from([
                ("schema".into(), Value::String("scene".into())),
                ("items".into(), Value::List(items)),
            ]))
        };
        let parsed = scene_block(&scene(vec![scene_item.clone()])).unwrap();
        assert_eq!(parsed[0].frames, ["terrain.grass"]);
        assert_eq!(parsed[0].anchor, Position { x: 1, y: 2 });
        assert!(scene_block(&scene(vec![scene_item.clone(), scene_item])).is_err());
    }
}
