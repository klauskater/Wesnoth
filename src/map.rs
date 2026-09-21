//! Map resource loading, independent of scenario orchestration.
use crate::{
    engine::{Map, Position},
    terrain_scene::TerrainScript,
    value::Value,
    wml::{self, Node},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapTile {
    pub color: [u8; 3],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapTiles(BTreeMap<String, MapTile>);

impl MapTiles {
    pub fn get(&self, terrain_code: &str) -> Option<&MapTile> {
        let terrain = crate::terrain::gameplay_type(terrain_code);
        self.0.get(terrain).or_else(|| {
            self.0.get(match terrain {
                "mountains" => "hills",
                "deep_water" | "swamp_water" => "water",
                _ => terrain,
            })
        })
    }
}

/// Собирает игровую карту из WML-узла `[map]`.
///
/// Функция публична, чтобы отладочные инструменты читали карту точно так же,
/// как игра, не заводя второй парсер формата `data=<<...>>`.
pub fn load_map(node: &Node) -> Result<Map, String> {
    let width = usize::try_from(dimension(node, "width")?)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "[map].width must be a positive integer".to_owned())?;
    let height = usize::try_from(dimension(node, "height")?)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "[map].height must be a positive integer".to_owned())?;
    let rows: Vec<Vec<String>> = node
        .attribute("data")?
        .lines()
        .map(|line| line.split(',').map(|cell| cell.trim().to_owned()).collect())
        .collect();
    if rows.len() != height || rows.iter().any(|row| row.len() != width) {
        return Err(format!("map data does not match {width}x{height}"));
    }
    Ok(Map {
        width,
        height,
        cells: rows
            .into_iter()
            .flatten()
            .map(|terrain| Value::Map(BTreeMap::from([("terrain".into(), Value::String(terrain))])))
            .collect(),
    })
}

/// Возвращает код рельефа из данных ячейки текущего формата приключений.
///
/// Этот метод намеренно находится в загрузчике карты, а не в хранилище движка:
/// поле `terrain` является соглашением ресурсов игры, неизвестным `Store`.
impl Map {
    pub fn raw(&self, position: Position) -> Result<&str, String> {
        terrain_code(self.cell(position)?)
    }
}

pub fn terrain_code(cell: &Value) -> Result<&str, String> {
    cell.as_str()
        .or_else(|| cell.get("terrain").and_then(Value::as_str))
        .ok_or_else(|| "map cell has no string terrain field".to_owned())
}

fn parse_color(value: &str) -> Result<[u8; 3], String> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.is_ascii() {
        return Err(format!("invalid map object color: {value}"));
    }
    let channel = |start| {
        u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| format!("invalid map object color: {value}"))
    };
    Ok([channel(0)?, channel(2)?, channel(4)?])
}

pub fn load_resources(
    map: &Map,
    paths: &str,
    read: &impl Fn(&str) -> Result<String, String>,
) -> Result<(MapTiles, Vec<TerrainScript>), String> {
    let mut map_tiles = MapTiles::default();
    let mut terrain_scripts = Vec::new();
    for path in paths
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        let roots = wml::parse(&read(path)?)?;
        let mut objects = roots.iter().filter(|node| node.name == "map_object");
        let map_object = objects.next().ok_or("missing [map_object]")?;
        if objects.next().is_some() {
            return Err("expected one [map_object]".into());
        }
        let id = map_object.attribute("id")?.to_owned();
        let color = map_object
            .attributes
            .get("color")
            .map(|value| parse_color(value))
            .transpose()?
            .unwrap_or([96, 96, 96]);
        if map_tiles.0.insert(id.clone(), MapTile { color }).is_some() {
            return Err(format!("duplicate map object id: {id}"));
        }
        if let Some(renderer) = map_object.attributes.get("renderer") {
            terrain_scripts.push(TerrainScript {
                family: id,
                codes: map_object
                    .attribute("codes")?
                    .split(',')
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                    .map(str::to_owned)
                    .collect(),
                source: read(renderer)
                    .map_err(|error| format!("cannot read terrain renderer {renderer}: {error}"))?,
            });
        }
    }
    for cell in &map.cells {
        let terrain = terrain_code(cell)?;
        if map_tiles.get(terrain).is_none() {
            return Err(format!("map uses unknown map object: {terrain}"));
        }
    }

    Ok((map_tiles, terrain_scripts))
}

fn dimension(node: &Node, name: &str) -> Result<i64, String> {
    node.attribute(name)?
        .parse()
        .map_err(|_| format!("[map].{name} must be an integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_map_dimensions() {
        for dimensions in ["width=0\nheight=1", "width=1\nheight=-1"] {
            let source = format!("[map]\n{dimensions}\ndata=<<\ngrassland\n>>\n[/map]");
            let nodes = wml::parse(&source).unwrap();
            assert!(load_map(&nodes[0]).is_err());
        }
    }
}
