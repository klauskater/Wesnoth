//! Batched weighted hex search with no game-rule callbacks or terrain semantics.
//!
//! Owns temporary algorithm data only. Contract:
//! `contracts/target/modules/engine/pathfinding.md`.

use super::hex::{Bounds, Position};
use mlua::{Lua, Table};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap},
};

pub struct SearchRequest {
    pub bounds: Bounds,
    pub origin: Position,
    pub budget: i64,
    pub costs: BTreeMap<Position, i64>,
    pub blocked: BTreeSet<Position>,
    pub stop_cells: BTreeSet<Position>,
}

#[derive(Debug, PartialEq)]
pub struct SearchResult {
    pub costs: Vec<(Position, i64)>,
    pub predecessors: Vec<(Position, Position)>,
}

pub fn search(request: &SearchRequest) -> Result<SearchResult, String> {
    if request.budget < 0 || !request.bounds.contains(request.origin) {
        return Err("search origin and budget are invalid".into());
    }
    for y in 1..=request.bounds.height as i64 {
        for x in 1..=request.bounds.width as i64 {
            let position = Position { x, y };
            if !request.blocked.contains(&position)
                && request.costs.get(&position).is_none_or(|cost| *cost < 0)
            {
                return Err(format!("missing or negative cost at ({x}, {y})"));
            }
        }
    }
    if request
        .blocked
        .iter()
        .chain(&request.stop_cells)
        .any(|position| !request.bounds.contains(*position))
    {
        return Err("search constraint is outside bounds".into());
    }

    let mut best = BTreeMap::from([(request.origin, 0)]);
    let mut predecessors = BTreeMap::new();
    let mut finalized = BTreeSet::new();
    let mut frontier = BinaryHeap::from([Reverse((0, 0usize, request.origin))]);
    let mut serial = 0usize;
    let mut costs = Vec::new();
    while let Some(Reverse((cost, _, current))) = frontier.pop() {
        if !finalized.insert(current) {
            continue;
        }
        costs.push((current, cost));
        if request.stop_cells.contains(&current) || cost >= request.budget {
            continue;
        }
        for neighbor in request.bounds.neighbors(current)? {
            if request.blocked.contains(&neighbor) || finalized.contains(&neighbor) {
                continue;
            }
            let step = request.costs[&neighbor];
            let next_cost = cost.saturating_add(step).min(request.budget);
            if best.get(&neighbor).is_some_and(|best| next_cost >= *best) {
                continue;
            }
            best.insert(neighbor, next_cost);
            predecessors.insert(neighbor, current);
            serial = serial.saturating_add(1);
            frontier.push(Reverse((next_cost, serial, neighbor)));
        }
    }
    Ok(SearchResult {
        costs,
        predecessors: predecessors.into_iter().collect(),
    })
}

fn read_position(table: Table) -> mlua::Result<Position> {
    Ok(Position {
        x: table.get("x")?,
        y: table.get("y")?,
    })
}

fn positions(table: &Table, key: &str) -> mlua::Result<BTreeSet<Position>> {
    match table.get::<Option<Table>>(key)? {
        Some(list) => list
            .sequence_values::<Table>()
            .map(|item| read_position(item?))
            .collect(),
        None => Ok(BTreeSet::new()),
    }
}

fn write_position(lua: &Lua, position: Position) -> mlua::Result<Table> {
    let table = lua.create_table_with_capacity(0, 2)?;
    table.set("x", position.x)?;
    table.set("y", position.y)?;
    Ok(table)
}

pub(crate) fn search_lua(lua: &Lua, bounds: Bounds, table: Table) -> mlua::Result<Table> {
    let mut costs = BTreeMap::new();
    for item in table.get::<Table>("costs")?.sequence_values::<Table>() {
        let item = item?;
        costs.insert(read_position(item.get("position")?)?, item.get("cost")?);
    }
    let result = search(&SearchRequest {
        bounds,
        origin: read_position(table.get("origin")?)?,
        budget: table.get("budget")?,
        costs,
        blocked: positions(&table, "blocked")?,
        stop_cells: positions(&table, "stop_cells")?,
    })
    .map_err(mlua::Error::runtime)?;
    let output = lua.create_table_with_capacity(0, 2)?;
    let output_costs = lua.create_table_with_capacity(result.costs.len(), 0)?;
    for (index, (position, cost)) in result.costs.into_iter().enumerate() {
        let item = lua.create_table_with_capacity(0, 2)?;
        item.set("position", write_position(lua, position)?)?;
        item.set("cost", cost)?;
        output_costs.raw_set(index + 1, item)?;
    }
    output.set("costs", output_costs)?;
    let output_predecessors = lua.create_table_with_capacity(result.predecessors.len(), 0)?;
    for (index, (position, predecessor)) in result.predecessors.into_iter().enumerate() {
        let item = lua.create_table_with_capacity(0, 2)?;
        item.set("position", write_position(lua, position)?)?;
        item.set("predecessor", write_position(lua, predecessor)?)?;
        output_predecessors.raw_set(index + 1, item)?;
    }
    output.set("predecessors", output_predecessors)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_supplied_costs_and_stops_control_the_search() {
        let bounds = Bounds::new(3, 1).unwrap();
        let request = |middle| SearchRequest {
            bounds,
            origin: Position { x: 1, y: 1 },
            budget: 4,
            costs: BTreeMap::from([
                (Position { x: 1, y: 1 }, 0),
                (Position { x: 2, y: 1 }, middle),
                (Position { x: 3, y: 1 }, 1),
            ]),
            blocked: BTreeSet::new(),
            stop_cells: BTreeSet::new(),
        };
        assert_eq!(search(&request(9)).unwrap().costs.len(), 2);
        assert_eq!(search(&request(1)).unwrap().costs.len(), 3);
    }
}
