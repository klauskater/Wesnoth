use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use super::{CampaignState, DialogLine, EngineResources, Initialization};
use crate::{
    adventure::{Adventure, AdventureScenario},
    engine::{
        World,
        resources::Package,
        session::SessionInput,
    },
    map::{MapTiles, load_map},
    terrain_scene::TerrainScene,
    value::Value,
    wml::{self, Node},
};

struct ResourceSelection<'a> {
    process: &'a str,
    map: &'a str,
    map_objects: &'a str,
    unit_catalogs: Vec<&'a str>,
    dialogs: &'a str,
    game_rules: &'a str,
    combat_rules: &'a str,
}

pub(super) fn filesystem(
    scripts: impl AsRef<Path>,
) -> Result<impl for<'a> Fn(&'a str) -> Result<String, String>, String> {
    let scripts = fs::canonicalize(scripts.as_ref())
        .map_err(|error| format!("cannot open scripts directory: {error}"))?;
    Ok(move |path: &str| {
        let requested = Path::new(path);
        if requested.is_absolute() {
            return Err(format!("resource path must be relative: {path}"));
        }
        let full = fs::canonicalize(scripts.join(requested))
            .map_err(|error| format!("cannot read {path}: {error}"))?;
        if !full.starts_with(&scripts) {
            return Err(format!("resource path escapes scripts directory: {path}"));
        }
        fs::read_to_string(full).map_err(|error| format!("cannot read {path}: {error}"))
    })
}

pub(super) fn scenario(
    scenario_path: &str,
    campaign: Option<&CampaignState>,
    read: &impl Fn(&str) -> Result<String, String>,
    saved: Option<&[u8]>,
) -> Result<EngineResources, String> {
    // Проверяем путь через границу пакета до прямого чтения WML. Помимо защиты
    // это сохраняет единый текст ошибки для файлового и встроенного источника.
    Package::open(read)?.read_text(scenario_path)?;
    let scenario_document = read_wml(scenario_path, read)?;
    let scenario = one_root(&scenario_document, "scenario")?;
    let resources = scenario.child("resources")?;
    let rules = split_paths(resources.attribute("rules")?)
        .map(lua_path_to_module)
        .collect::<Result<Vec<_>, _>>()?;
    build(
        ResourceSelection {
            process: scenario_path,
            map: resources.attribute("map")?,
            map_objects: resources.attribute("map_objects")?,
            unit_catalogs: split_paths(resources.attribute("unit_types")?).collect(),
            dialogs: resources.attribute("dialogs")?,
            game_rules: rules
                .iter()
                .find(|module| module.as_str() == "rules.basic_combat")
                .map(String::as_str)
                .unwrap_or("rules.basic_combat"),
            combat_rules: "game.rules.combat",
        },
        campaign,
        read,
        saved,
    )
}

/// Собирает главу, начиная с манифеста приключения.
///
/// В отличие от временного сценарного пути выше, здесь сценарий не объявляет
/// собственные ресурсы: состав юнитов и правила принадлежат приключению, карта
/// объявляет свои тайлы, а сценарий содержит только игровой процесс.
pub(super) fn adventure(
    adventure: &Adventure,
    chapter: &AdventureScenario,
    campaign: Option<&CampaignState>,
    read: &impl Fn(&str) -> Result<String, String>,
    saved: Option<&[u8]>,
) -> Result<EngineResources, String> {
    let map_document = read_wml(&chapter.map, read)?;
    let map = one_root(&map_document, "map")?;
    let tiles = map.child("tiles")?.attribute("sources")?;
    build(
        ResourceSelection {
            process: &chapter.process,
            map: &chapter.map,
            map_objects: tiles,
            unit_catalogs: adventure.unit_catalogs.iter().map(String::as_str).collect(),
            dialogs: &chapter.dialogs,
            game_rules: &adventure.rules.game,
            combat_rules: &adventure.rules.combat,
        },
        campaign,
        read,
        saved,
    )
}

fn build(
    selected: ResourceSelection<'_>,
    campaign: Option<&CampaignState>,
    read: &impl Fn(&str) -> Result<String, String>,
    saved: Option<&[u8]>,
) -> Result<EngineResources, String> {
    let package = Package::open(read)?;
    let package_identity = package.manifest().identity();
    let mut scene_assets: BTreeMap<_, _> = package
        .manifest()
        .assets
        .iter()
        .map(|(id, asset)| (id.clone(), asset.path.clone()))
        .collect();
    let mut package_assets: BTreeSet<_> = scene_assets.keys().cloned().collect();
    let entry = package.manifest().entry.clone();
    let module_sources = package.module_sources()?;
    let read = &|path: &str| package.read_text(path);

    let scenario_path = selected.process;
    let scenario_document = read_wml(selected.process, read)?;
    let scenario = one_root(&scenario_document, "scenario")?;
    let map_document = read_wml(selected.map, read)?;
    let map = load_map(one_root(&map_document, "map")?)?;
    let (map_tiles, terrain_scripts) =
        crate::map::load_resources(&map, selected.map_objects, read)?;
    let terrain_scene = TerrainScene::from_lua(&map, &terrain_scripts)?;
    package_assets.extend(terrain_scene.assets().keys().cloned());
    scene_assets.extend(terrain_scene.assets().clone());

    let catalogs = load_catalog_files(selected.unit_catalogs.iter().copied(), read)?;
    let dialogs = load_dialogs(&read_wml(selected.dialogs, read)?)?;
    package.module(selected.game_rules)?;
    package.module(selected.combat_rules)?;

    let seed = parse_i64(scenario, "random_seed")? as u64;
    let mut request = BTreeMap::from([
        ("scenario".into(), Value::Map(node_properties(scenario))),
        ("catalogs".into(), Value::List(catalogs)),
        ("scenario_path".into(), Value::String(scenario_path.into())),
        ("scene".into(), terrain_scene.presentation()),
        ("map".into(), map_value(&map, &map_tiles)),
        (
            "assets".into(),
            assets_value(&scene_assets, &package.manifest().assets),
        ),
        ("dialogs".into(), dialogs_value(&dialogs)),
        (
            "rules".into(),
            Value::Map(BTreeMap::from([
                ("game".into(), Value::String(selected.game_rules.into())),
                ("combat".into(), Value::String(selected.combat_rules.into())),
            ])),
        ),
    ]);
    if let Some(campaign) = campaign {
        request.insert(
            "carryover".into(),
            Value::Map(BTreeMap::from([
                ("units".into(), Value::List(campaign.units.clone())),
                ("gold".into(), Value::Integer(campaign.gold)),
                ("variables".into(), Value::Map(campaign.variables.clone())),
            ])),
        );
    }
    let request = Value::Map(request);
    let world = World {
        map,
        entities: BTreeMap::new(),
        data: BTreeMap::new(),
    };
    let session_id = format!("{}:{scenario_path}", package_identity.package_id);
    let input = SessionInput {
        id: session_id,
        package: package_identity,
        assets: package_assets,
        world,
        seed,
        modules: module_sources,
        entry,
    };
    Ok(EngineResources {
        id: scenario.attribute("id")?.into(),
        name: scenario.attribute("name")?.into(),
        start_dialog: scenario.attribute("on_start_dialog")?.into(),
        session: input,
        initialization: match saved {
            Some(bytes) => Initialization::Restore(bytes.to_vec()),
            None => Initialization::New(request),
        },
        map_tiles,
        terrain_scripts,
        scene_assets,
        dialogs,
    })
}

fn load_catalog_files<'a>(
    paths: impl IntoIterator<Item = &'a str>,
    read: &impl Fn(&str) -> Result<String, String>,
) -> Result<Vec<Value>, String> {
    let mut catalogs = Vec::new();
    for path in paths {
        for root in &read_wml(path, read)? {
            let mut value = node_properties(root);
            value.insert("__tag".into(), Value::String(root.name.clone()));
            catalogs.push(Value::Map(value));
        }
    }
    Ok(catalogs)
}

fn map_value(map: &crate::engine::Map, tiles: &MapTiles) -> Value {
    let cells = (1..=map.height as i64)
        .flat_map(|y| (1..=map.width as i64).map(move |x| (x, y)))
        .map(|(x, y)| {
            let code = map.raw(crate::engine::Position { x, y }).unwrap();
            let color = tiles.get(code).map_or([255, 0, 255], |tile| tile.color);
            Value::Map(BTreeMap::from([
                (
                    "position".into(),
                    Value::Map(BTreeMap::from([
                        ("x".into(), Value::Integer(x)),
                        ("y".into(), Value::Integer(y)),
                    ])),
                ),
                ("code".into(), Value::String(code.into())),
                (
                    "color".into(),
                    Value::List(
                        color
                            .into_iter()
                            .map(|v| Value::Integer(v.into()))
                            .collect(),
                    ),
                ),
            ]))
        })
        .collect();
    Value::Map(BTreeMap::from([
        ("schema".into(), Value::String("map".into())),
        ("cells".into(), Value::List(cells)),
    ]))
}

fn assets_value(
    assets: &BTreeMap<String, String>,
    descriptors: &BTreeMap<String, crate::engine::resources::AssetDescriptor>,
) -> Value {
    Value::Map(BTreeMap::from([
        ("schema".into(), Value::String("assets".into())),
        (
            "items".into(),
            Value::List(
                assets
                    .iter()
                    .map(|(id, path)| {
                        let mut item = BTreeMap::from([
                            ("id".into(), Value::String(id.clone())),
                            ("path".into(), Value::String(path.clone())),
                        ]);
                        if let Some(asset) = descriptors.get(id) {
                            item.insert(
                                "media_type".into(),
                                Value::String(asset.media_type.clone()),
                            );
                        }
                        Value::Map(item)
                    })
                    .collect(),
            ),
        ),
    ]))
}

fn dialogs_value(dialogs: &BTreeMap<String, Vec<DialogLine>>) -> Value {
    Value::Map(
        dialogs
            .iter()
            .map(|(id, lines)| {
                let lines = lines
                    .iter()
                    .map(|line| {
                        Value::Map(BTreeMap::from([
                            ("speaker".into(), Value::String(line.speaker.clone())),
                            ("text".into(), Value::String(line.text.clone())),
                        ]))
                    })
                    .collect();
                (id.clone(), Value::List(lines))
            })
            .collect(),
    )
}

fn lua_path_to_module(path: &str) -> Result<String, String> {
    Ok(path
        .strip_suffix(".lua")
        .ok_or_else(|| format!("rule module must end with .lua: {path}"))?
        .replace('/', "."))
}

fn read_wml(
    path: &str,
    read: &impl Fn(&str) -> Result<String, String>,
) -> Result<Vec<Node>, String> {
    wml::parse(&read(path)?).map_err(|error| format!("{path}: {error}"))
}

fn one_root<'a>(nodes: &'a [Node], name: &str) -> Result<&'a Node, String> {
    let mut matching = nodes.iter().filter(|node| node.name == name);
    let node = matching.next().ok_or_else(|| format!("missing [{name}]"))?;
    if matching.next().is_some() {
        return Err(format!("expected one [{name}]"));
    }
    Ok(node)
}

fn split_paths(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_i64(node: &Node, name: &str) -> Result<i64, String> {
    node.attribute(name)?
        .parse()
        .map_err(|_| format!("[{}].{name} must be an integer", node.name))
}

fn node_properties(node: &Node) -> BTreeMap<String, Value> {
    let mut values = node
        .attributes
        .iter()
        .map(|(key, value)| {
            let value = value
                .parse::<i64>()
                .map(Value::Integer)
                .unwrap_or_else(|_| Value::String(value.clone()));
            (key.clone(), value)
        })
        .collect::<BTreeMap<_, _>>();
    let mut children: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for child in &node.children {
        children
            .entry(child.name.clone())
            .or_default()
            .push(Value::Map(node_properties(child)));
    }
    if !children.is_empty() {
        values.insert(
            "__children".into(),
            Value::Map(
                children
                    .into_iter()
                    .map(|(name, nodes)| (name, Value::List(nodes)))
                    .collect(),
            ),
        );
    }
    values
}

fn load_dialogs(nodes: &[Node]) -> Result<BTreeMap<String, Vec<DialogLine>>, String> {
    let mut dialogs = BTreeMap::new();
    for dialog in nodes.iter().filter(|node| node.name == "dialog") {
        let id = dialog.attribute("id")?.to_owned();
        let lines = dialog
            .children_named("line")
            .map(|line| {
                Ok(DialogLine {
                    speaker: line.attribute("speaker")?.into(),
                    text: line.attribute("text")?.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if lines.is_empty() {
            return Err(format!("dialog {id} has no lines"));
        }
        if dialogs.insert(id.clone(), lines).is_some() {
            return Err(format!("duplicate dialog id: {id}"));
        }
    }
    Ok(dialogs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfers_arbitrary_wml_properties_without_a_schema() {
        let nodes = wml::parse(
            "[thing]\nanswer=42\nlabel=custom\n[extra]\nflag=yes\n[/extra]\n[extra]\nflag=no\n[/extra]\n[/thing]",
        )
        .unwrap();
        let properties = node_properties(&nodes[0]);
        assert_eq!(properties["answer"], Value::Integer(42));
        assert_eq!(properties["label"], Value::String("custom".into()));
        let children = properties["__children"].as_map().unwrap();
        assert!(matches!(&children["extra"], Value::List(values) if values.len() == 2));
    }
}
