//! Generic weighted hex search. Game rules supply costs, blockers and stop zones;
//! the map is borrowed in place and the search never calls back into Lua.
use crate::engine::{Map, Position};
use mlua::{Lua, Table};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
};

pub struct SearchRequest {
    pub start: Position,
    pub budget: i64,
    pub max_step: i64,
    pub costs: BTreeMap<String, i64>,
    pub blocked: Vec<Position>,
    pub occupied: Vec<Position>,
    pub stop_near: Vec<Position>,
    pub destination: Option<Position>,
    pub paths: bool,
}

#[derive(Debug, PartialEq)]
pub struct Reachable {
    pub position: Position,
    pub cost: i64,
    pub stopped: bool,
    pub path: Option<Vec<Position>>,
}

pub fn search(map: &Map, request: &SearchRequest) -> Result<Vec<Reachable>, String> {
    if request.budget < 0 || request.max_step < 0 || request.costs.values().any(|cost| *cost <= 0) {
        return Err("movement budgets must be nonnegative and terrain costs positive".into());
    }
    let index = |p: Position| -> Result<usize, String> {
        map.raw(p)?;
        Ok((p.y as usize - 1) * map.width + p.x as usize - 1)
    };
    let position = |i: usize| Position {
        x: (i % map.width + 1) as i64,
        y: (i / map.width + 1) as i64,
    };
    let start = index(request.start)?;
    let destination = request.destination.map(index).transpose()?;
    let mut blocked = vec![false; map.cells.len()];
    let mut occupied = blocked.clone();
    let mut stop = blocked.clone();
    for p in &request.blocked {
        blocked[index(*p)?] = true;
    }
    for p in &request.occupied {
        occupied[index(*p)?] = true;
    }
    for p in &request.stop_near {
        index(*p)?;
        for adjacent in map.neighbors_iter(*p) {
            stop[index(adjacent)?] = true;
        }
    }
    stop[start] = false;
    let mut best = vec![i64::MAX; map.cells.len()];
    let mut parent = vec![None; map.cells.len()];
    let mut finalized = vec![false; map.cells.len()];
    // Serial numbers retain the Lua search's FIFO ordering for equal-cost routes.
    let mut frontier = BinaryHeap::from([Reverse((0, 0usize, start))]);
    let mut serial = 0;
    best[start] = 0;
    let mut result = Vec::new();
    while let Some(Reverse((cost, _, current))) = frontier.pop() {
        if finalized[current] {
            continue;
        }
        finalized[current] = true;
        if current != start
            && !occupied[current]
            && destination.is_none_or(|target| target == current)
        {
            let path = if request.paths {
                let mut path = Vec::new();
                let mut cursor = current;
                while cursor != start {
                    path.push(position(cursor));
                    cursor = parent[cursor].expect("reachable cell has a predecessor");
                }
                path.reverse();
                Some(path)
            } else {
                None
            };
            result.push(Reachable {
                position: position(current),
                cost,
                stopped: stop[current],
                path,
            });
        }
        if destination == Some(current) {
            break;
        }
        if stop[current] || cost >= request.budget {
            continue;
        }
        for neighbor in map.neighbors_iter(position(current)) {
            let next = index(neighbor)?;
            if blocked[next] || finalized[next] {
                continue;
            }
            let Some(&step) = request.costs.get(map.get(neighbor)?) else {
                continue;
            };
            if step > request.max_step {
                continue;
            }
            let next_cost = cost.saturating_add(step).min(request.budget);
            if next_cost >= best[next] {
                continue;
            }
            best[next] = next_cost;
            parent[next] = Some(current);
            serial += 1;
            frontier.push(Reverse((next_cost, serial, next)));
        }
    }
    Ok(result)
}

fn read_position(table: Table) -> mlua::Result<Position> {
    Ok(Position {
        x: table.get("x")?,
        y: table.get("y")?,
    })
}
fn positions(table: &Table, key: &str) -> mlua::Result<Vec<Position>> {
    match table.get::<Option<Table>>(key)? {
        Some(list) => list
            .sequence_values::<Table>()
            .map(|item| read_position(item?))
            .collect(),
        None => Ok(Vec::new()),
    }
}
fn write_position(lua: &Lua, p: Position) -> mlua::Result<Table> {
    let table = lua.create_table_with_capacity(0, 2)?;
    table.set("x", p.x)?;
    table.set("y", p.y)?;
    Ok(table)
}

pub(crate) fn search_lua(lua: &Lua, map: &Map, table: Table) -> mlua::Result<Table> {
    let request = SearchRequest {
        start: read_position(table.get("start")?)?,
        budget: table.get("budget")?,
        max_step: table.get("max_step")?,
        costs: table
            .get::<Table>("costs")?
            .pairs::<String, i64>()
            .collect::<mlua::Result<_>>()?,
        blocked: positions(&table, "blocked")?,
        occupied: positions(&table, "occupied")?,
        stop_near: positions(&table, "stop_near")?,
        destination: table
            .get::<Option<Table>>("destination")?
            .map(read_position)
            .transpose()?,
        paths: table.get::<Option<bool>>("paths")?.unwrap_or(true),
    };
    let cells = search(map, &request).map_err(mlua::Error::runtime)?;
    let result = lua.create_table_with_capacity(cells.len(), 0)?;
    for (i, cell) in cells.into_iter().enumerate() {
        let entry = lua.create_table_with_capacity(0, 4)?;
        entry.set("position", write_position(lua, cell.position)?)?;
        entry.set("cost", cell.cost)?;
        entry.set("zoc", cell.stopped)?;
        if let Some(path) = cell.path {
            let output = lua.create_table_with_capacity(path.len(), 0)?;
            for (j, p) in path.into_iter().enumerate() {
                output.raw_set(j + 1, write_position(lua, p)?)?;
            }
            entry.set("path", output)?;
        }
        result.raw_set(i + 1, entry)?;
    }
    Ok(result)
}
