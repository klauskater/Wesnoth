//! Authoritative mutable game data and transactional RNG.
//!
//! The store does not interpret game fields or hold UI state. Contract:
//! `contracts/target/modules/engine/store.md`.

use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use serde::{Deserialize, Serialize};

use super::{hex, value::Value};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<String>,
}

impl Map {
    pub fn bounds(&self) -> Result<hex::Bounds, String> {
        hex::Bounds::new(self.width, self.height)
    }

    pub fn raw(&self, position: hex::Position) -> Result<&str, String> {
        if !self.bounds()?.contains(position) {
            return Err(format!(
                "position ({}, {}) is outside the map",
                position.x, position.y
            ));
        }
        let index = (position.y as usize - 1) * self.width + position.x as usize - 1;
        self.cells
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| "map dimensions do not match its cells".to_owned())
    }

    pub fn are_adjacent(&self, a: hex::Position, b: hex::Position) -> bool {
        hex::adjacent(a, b)
    }

    pub fn neighbors(&self, position: hex::Position) -> Vec<hex::Position> {
        self.neighbors_iter(position).collect()
    }

    pub fn neighbors_iter(
        &self,
        position: hex::Position,
    ) -> impl Iterator<Item = hex::Position> + '_ {
        let bounds = hex::Bounds {
            width: self.width,
            height: self.height,
        };
        hex::neighbors(position)
            .into_iter()
            .filter(move |candidate| bounds.contains(*candidate))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Object {
    pub id: String,
    pub properties: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct World {
    pub map: Map,
    pub objects: BTreeMap<String, Object>,
    pub state: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct DeterministicRandom {
    pub(crate) state: u64,
}

impl DeterministicRandom {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub(crate) fn integer(&mut self, min: i64, max: i64) -> Result<i64, String> {
        if min > max {
            return Err(format!("invalid random range {min}..{max}"));
        }
        let span = max
            .checked_sub(min)
            .and_then(|distance| distance.checked_add(1))
            .ok_or_else(|| format!("random range is too wide: {min}..{max}"))?;
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        Ok(min + ((self.state >> 32) % span as u64) as i64)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Address {
    pub collection: String,
    pub id: String,
}

impl Address {
    pub fn new(collection: impl Into<String>, id: impl Into<String>) -> Result<Self, String> {
        let address = Self {
            collection: collection.into(),
            id: id.into(),
        };
        if address.collection.is_empty() || address.id.is_empty() {
            return Err("store address parts must not be empty".into());
        }
        Ok(address)
    }

    pub fn object(id: impl Into<String>) -> Self {
        Self {
            collection: "objects".into(),
            id: id.into(),
        }
    }

    pub fn state(id: impl Into<String>) -> Self {
        Self {
            collection: "state".into(),
            id: id.into(),
        }
    }

    pub fn map(position: hex::Position) -> Self {
        Self {
            collection: "map".into(),
            id: format!("{},{}", position.x, position.y),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Insert {
        address: Address,
        value: Value,
    },
    Update {
        address: Address,
        value: Value,
    },
    Remove {
        address: Address,
    },
    SetField {
        address: Address,
        field: String,
        value: Value,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub address: Address,
    pub value: Value,
}

#[derive(Clone)]
pub struct ReadView {
    world: Rc<World>,
    revision: u64,
}

impl ReadView {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn get(&self, address: &Address) -> Result<Option<Value>, String> {
        read_value(&self.world, address)
    }

    pub fn select(
        &self,
        addresses: impl IntoIterator<Item = Address>,
        fields: Option<&[String]>,
    ) -> Result<Vec<Record>, String> {
        select(&self.world, addresses, fields)
    }
}

#[derive(Clone)]
pub struct Transaction {
    expected_revision: u64,
    world: Rc<World>,
    random: DeterministicRandom,
    changes: BTreeSet<Address>,
    random_changed: bool,
}

impl Transaction {
    pub fn revision(&self) -> u64 {
        self.expected_revision
    }

    pub fn get(&self, address: &Address) -> Result<Option<Value>, String> {
        read_value(&self.world, address)
    }

    pub fn select(
        &self,
        addresses: impl IntoIterator<Item = Address>,
        fields: Option<&[String]>,
    ) -> Result<Vec<Record>, String> {
        select(&self.world, addresses, fields)
    }

    pub fn apply(&mut self, operations: &[Operation]) -> Result<(), String> {
        let mut candidate = self.clone();
        for operation in operations {
            candidate.apply_one(operation)?;
        }
        *self = candidate;
        Ok(())
    }

    pub fn random(&mut self, min: i64, max: i64) -> Result<i64, String> {
        let value = self.random.integer(min, max)?;
        self.random_changed = true;
        Ok(value)
    }

    pub fn rollback(self) {}

    pub(crate) fn map(&self) -> &Map {
        &self.world.map
    }

    pub(crate) fn object(&self, id: &str) -> Option<&Object> {
        self.world.objects.get(id)
    }

    pub(crate) fn objects(&self) -> impl Iterator<Item = &Object> {
        self.world.objects.values()
    }

    pub(crate) fn state(&self, id: &str) -> Option<&Value> {
        self.world.state.get(id)
    }

    pub(crate) fn states(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.world
            .state
            .iter()
            .map(|(key, value)| (key.as_str(), value))
    }

    fn apply_one(&mut self, operation: &Operation) -> Result<(), String> {
        match operation {
            Operation::Insert { address, value } => {
                if read_value(&self.world, address)?.is_some() {
                    return Err(format!("store record already exists: {address:?}"));
                }
                write_value(&mut self.world, address, value.clone(), true)?;
                self.changes.insert(address.clone());
            }
            Operation::Update { address, value } => {
                let current = read_value(&self.world, address)?
                    .ok_or_else(|| format!("store record not found: {address:?}"))?;
                if &current != value {
                    write_value(&mut self.world, address, value.clone(), false)?;
                    self.changes.insert(address.clone());
                }
            }
            Operation::Remove { address } => {
                if read_value(&self.world, address)?.is_none() {
                    return Err(format!("store record not found: {address:?}"));
                }
                remove_value(&mut self.world, address)?;
                self.changes.insert(address.clone());
            }
            Operation::SetField {
                address,
                field,
                value,
            } => {
                if field.is_empty() {
                    return Err("store field must not be empty".into());
                }
                if read_field(&self.world, address, field)? != Some(value) {
                    set_field(&mut self.world, address, field, value.clone())?;
                    self.changes.insert(address.clone());
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub revision: u64,
    pub changed_addresses: Vec<Address>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StoreSnapshot {
    pub world: World,
    pub random_state: u64,
    pub revision: u64,
}

#[derive(Clone)]
pub struct Store {
    world: Rc<World>,
    random: DeterministicRandom,
    revision: u64,
}

impl Store {
    pub fn new(world: World, seed: u64) -> Result<Self, String> {
        world.map.bounds()?;
        if world.map.cells.len() != world.map.width.saturating_mul(world.map.height) {
            return Err("map dimensions do not match its cells".into());
        }
        if world
            .objects
            .iter()
            .any(|(id, object)| id.is_empty() || object.id != *id)
            || world.state.keys().any(String::is_empty)
        {
            return Err("world contains an invalid store id".into());
        }
        Ok(Self {
            world: Rc::new(world),
            random: DeterministicRandom::new(seed),
            revision: 0,
        })
    }

    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn read(&self) -> ReadView {
        ReadView {
            world: Rc::clone(&self.world),
            revision: self.revision,
        }
    }

    pub fn snapshot(&self) -> StoreSnapshot {
        StoreSnapshot {
            world: (*self.world).clone(),
            random_state: self.random.state,
            revision: self.revision,
        }
    }

    pub fn from_snapshot(snapshot: StoreSnapshot) -> Result<Self, String> {
        let mut store = Self::new(snapshot.world, snapshot.random_state)?;
        store.revision = snapshot.revision;
        Ok(store)
    }

    pub fn begin(&self, expected_revision: u64) -> Result<Transaction, String> {
        if expected_revision != self.revision {
            return Err(format!(
                "store revision conflict: expected {expected_revision}, current {}",
                self.revision
            ));
        }
        Ok(Transaction {
            expected_revision,
            world: Rc::clone(&self.world),
            random: self.random.clone(),
            changes: BTreeSet::new(),
            random_changed: false,
        })
    }

    pub(crate) fn commit(&mut self, transaction: Transaction) -> Result<Commit, String> {
        if transaction.expected_revision != self.revision {
            return Err(format!(
                "store revision conflict: expected {}, current {}",
                transaction.expected_revision, self.revision
            ));
        }
        if transaction.changes.is_empty() && !transaction.random_changed {
            return Ok(Commit {
                revision: self.revision,
                changed_addresses: Vec::new(),
            });
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| "store revision exhausted".to_owned())?;
        self.world = transaction.world;
        self.random = transaction.random;
        self.revision = revision;
        Ok(Commit {
            revision: self.revision,
            changed_addresses: transaction.changes.into_iter().collect(),
        })
    }
}

fn read_value(world: &World, address: &Address) -> Result<Option<Value>, String> {
    validate_address(address)?;
    match address.collection.as_str() {
        "objects" => Ok(world
            .objects
            .get(&address.id)
            .map(|object| Value::Map(object.properties.clone()))),
        "state" => Ok(world.state.get(&address.id).cloned()),
        "map" => {
            let position = parse_map_id(&address.id)?;
            Ok(Some(Value::String(world.map.raw(position)?.to_owned())))
        }
        collection => Err(format!("unknown store collection: {collection}")),
    }
}

fn read_field<'a>(
    world: &'a World,
    address: &Address,
    field: &str,
) -> Result<Option<&'a Value>, String> {
    validate_address(address)?;
    match address.collection.as_str() {
        "objects" => world
            .objects
            .get(&address.id)
            .map(|object| object.properties.get(field))
            .ok_or_else(|| format!("store record not found: {address:?}")),
        "state" => match world.state.get(&address.id) {
            Some(Value::Map(record)) => Ok(record.get(field)),
            Some(_) => Err(format!("store record is not a map: {address:?}")),
            None => Err(format!("store record not found: {address:?}")),
        },
        "map" => Err("map cell is not a record".into()),
        collection => Err(format!("unknown store collection: {collection}")),
    }
}

fn validate_address(address: &Address) -> Result<(), String> {
    if address.collection.is_empty() || address.id.is_empty() {
        return Err("store address parts must not be empty".into());
    }
    Ok(())
}

fn select(
    world: &World,
    addresses: impl IntoIterator<Item = Address>,
    fields: Option<&[String]>,
) -> Result<Vec<Record>, String> {
    let mut addresses: Vec<_> = addresses.into_iter().collect();
    addresses.sort();
    addresses.dedup();
    let mut records = Vec::new();
    for address in addresses {
        if let Some(mut value) = read_value(world, &address)? {
            if let (Some(fields), Value::Map(source)) = (fields, &value) {
                value = Value::Map(
                    fields
                        .iter()
                        .filter_map(|field| {
                            source
                                .get(field)
                                .cloned()
                                .map(|value| (field.clone(), value))
                        })
                        .collect(),
                );
            }
            records.push(Record { address, value });
        }
    }
    Ok(records)
}

fn write_value(
    world: &mut Rc<World>,
    address: &Address,
    value: Value,
    insert: bool,
) -> Result<(), String> {
    let world = Rc::make_mut(world);
    match address.collection.as_str() {
        "objects" => {
            let Value::Map(properties) = value else {
                return Err("object store values must be maps".into());
            };
            world.objects.insert(
                address.id.clone(),
                Object {
                    id: address.id.clone(),
                    properties,
                },
            );
        }
        "state" => {
            world.state.insert(address.id.clone(), value);
        }
        "map" => {
            if insert {
                return Err("map cells cannot be inserted".into());
            }
            let Value::String(value) = value else {
                return Err("map cell values must be strings".into());
            };
            let position = parse_map_id(&address.id)?;
            world.map.raw(position)?;
            let index = (position.y as usize - 1) * world.map.width + position.x as usize - 1;
            world.map.cells[index] = value;
        }
        collection => return Err(format!("unknown store collection: {collection}")),
    }
    Ok(())
}

fn remove_value(world: &mut Rc<World>, address: &Address) -> Result<(), String> {
    let world = Rc::make_mut(world);
    match address.collection.as_str() {
        "objects" => {
            world.objects.remove(&address.id);
        }
        "state" => {
            world.state.remove(&address.id);
        }
        "map" => return Err("map cells cannot be removed".into()),
        collection => return Err(format!("unknown store collection: {collection}")),
    }
    Ok(())
}

fn set_field(
    world: &mut Rc<World>,
    address: &Address,
    field: &str,
    value: Value,
) -> Result<(), String> {
    let world = Rc::make_mut(world);
    match address.collection.as_str() {
        "objects" => {
            world
                .objects
                .get_mut(&address.id)
                .expect("validated object address")
                .properties
                .insert(field.to_owned(), value);
        }
        "state" => {
            let Value::Map(record) = &mut world.state.get_mut(&address.id).unwrap() else {
                return Err(format!("store record is not a map: {address:?}"));
            };
            record.insert(field.to_owned(), value);
        }
        "map" => return Err("map cell is not a record".into()),
        collection => return Err(format!("unknown store collection: {collection}")),
    }
    Ok(())
}

fn parse_map_id(id: &str) -> Result<hex::Position, String> {
    let (x, y) = id
        .split_once(',')
        .ok_or_else(|| format!("invalid map address: {id}"))?;
    Ok(hex::Position {
        x: x.parse()
            .map_err(|_| format!("invalid map address: {id}"))?,
        y: y.parse()
            .map_err(|_| format!("invalid map address: {id}"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::new(
            World {
                map: Map {
                    width: 2,
                    height: 1,
                    cells: vec!["grassland".into(), "forest".into()],
                },
                objects: BTreeMap::from([(
                    "unit".into(),
                    Object {
                        id: "unit".into(),
                        properties: BTreeMap::from([("hp".into(), Value::Integer(10))]),
                    },
                )]),
                state: BTreeMap::from([("turn".into(), Value::Integer(1))]),
            },
            42,
        )
        .unwrap()
    }

    #[test]
    fn apply_is_atomic_and_rollback_preserves_rng() {
        let store = store();
        let mut transaction = store.begin(0).unwrap();
        let operations = [
            Operation::Insert {
                address: Address::state("temporary"),
                value: Value::Bool(true),
            },
            Operation::Remove {
                address: Address::state("missing"),
            },
        ];
        assert!(transaction.apply(&operations).is_err());
        assert_eq!(transaction.get(&Address::state("temporary")).unwrap(), None);
        let random = transaction.random(1, 100).unwrap();
        transaction.rollback();

        let mut fresh = store.begin(0).unwrap();
        assert_eq!(fresh.random(1, 100).unwrap(), random);
        assert_eq!(store.revision(), 0);
    }

    #[test]
    fn commit_tracks_stable_changes_noops_rng_and_conflicts() {
        let mut store = store();
        let mut first = store.begin(0).unwrap();
        let stale = store.begin(0).unwrap();
        first
            .apply(&[
                Operation::SetField {
                    address: Address::object("unit"),
                    field: "hp".into(),
                    value: Value::Integer(8),
                },
                Operation::Update {
                    address: Address::state("turn"),
                    value: Value::Integer(2),
                },
            ])
            .unwrap();
        assert_eq!(
            first
                .get(&Address::object("unit"))
                .unwrap()
                .unwrap()
                .get("hp"),
            Some(&Value::Integer(8))
        );
        let commit = store.commit(first).unwrap();
        assert_eq!(commit.revision, 1);
        assert_eq!(
            commit.changed_addresses,
            vec![Address::object("unit"), Address::state("turn")]
        );
        assert!(store.commit(stale).is_err());

        let mut noop = store.begin(1).unwrap();
        noop.apply(&[Operation::Update {
            address: Address::state("turn"),
            value: Value::Integer(2),
        }])
        .unwrap();
        assert_eq!(store.commit(noop).unwrap().revision, 1);

        let mut random = store.begin(1).unwrap();
        random.random(1, 10).unwrap();
        assert_eq!(store.commit(random).unwrap().revision, 2);
    }

    #[test]
    fn read_and_select_cover_named_collections_in_stable_order() {
        let store = store();
        let read = store.read();
        assert_eq!(read.revision(), 0);
        assert_eq!(
            read.get(&Address::map(hex::Position { x: 2, y: 1 }))
                .unwrap(),
            Some(Value::String("forest".into()))
        );
        let records = read
            .select(
                [Address::state("turn"), Address::object("unit")],
                Some(&["hp".into()]),
            )
            .unwrap();
        assert_eq!(records[0].address, Address::object("unit"));
        assert_eq!(records[1].address, Address::state("turn"));
        assert_eq!(
            records[0].value,
            Value::Map(BTreeMap::from([("hp".into(), Value::Integer(10))]))
        );
    }

    #[test]
    fn rejects_invalid_world_dimensions() {
        let world = World {
            map: Map {
                width: 1,
                height: 1,
                cells: Vec::new(),
            },
            objects: BTreeMap::new(),
            state: BTreeMap::new(),
        };
        assert!(Store::new(world, 1).is_err());
    }

    #[test]
    fn random_is_repeatable() {
        let mut a = DeterministicRandom::new(1);
        let mut b = DeterministicRandom::new(1);
        let left: Vec<_> = (0..5).map(|_| a.integer(1, 100).unwrap()).collect();
        let right: Vec<_> = (0..5).map(|_| b.integer(1, 100).unwrap()).collect();
        assert_eq!(left, right);
    }
}
