//! Sequential command orchestration and the sole Store commit owner.
//!
//! Game rules remain opaque to this module. Contract:
//! `contracts/target/modules/engine/session.md`.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    persistence,
    protocol::{
        self, Action, Command, CommandResult, CommandStatus, Effect, Error, Interaction,
        InteractionKind, Message, Presentation, ViewBlock, ViewSnapshot, ViewUpdate,
    },
    resources::PackageIdentity,
    runtime::{Runtime, RuntimeError},
    store::{Commit, Store, StoreSnapshot, World},
    value::Value,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Dispatch {
    pub result: CommandResult,
    pub events: Option<Value>,
    pub changes: Option<Commit>,
    pub view: Option<ViewUpdate>,
    pub presentation_error: Option<Error>,
}

#[derive(Clone)]
struct CachedCommand {
    viewer: String,
    command: Command,
    dispatch: Dispatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InteractionDelivery {
    pub interaction_id: String,
    pub view: Option<ViewUpdate>,
    pub command: Option<Dispatch>,
    pub error: Option<Error>,
}

#[derive(Clone)]
struct CachedInteraction {
    viewer: String,
    interaction: Interaction,
    delivery: InteractionDelivery,
}

#[derive(Default)]
struct Viewer {
    revision: u64,
    context: Option<Value>,
    blocks: BTreeMap<String, Value>,
}

pub struct Session {
    id: String,
    root_id: String,
    generation: u64,
    package: PackageIdentity,
    assets: BTreeSet<String>,
    runtime: Runtime,
    store: Store,
    commands: BTreeMap<String, CachedCommand>,
    viewers: BTreeMap<String, Viewer>,
    interactions: BTreeMap<String, CachedInteraction>,
}

impl Session {
    fn load_entry(
        id: impl Into<String>,
        package: PackageIdentity,
        assets: BTreeSet<String>,
        world: World,
        seed: u64,
        modules: BTreeMap<String, String>,
        entry: &str,
    ) -> Result<Self, String> {
        let id = id.into();
        if id.is_empty() {
            return Err("session id must not be empty".into());
        }
        if package.protocol_version != protocol::PROTOCOL_VERSION {
            return Err(format!(
                "incompatible package protocol: {} (expected {})",
                package.protocol_version,
                protocol::PROTOCOL_VERSION
            ));
        }
        let (runtime, store) = Runtime::load_entry(world, seed, modules, entry)?;
        Ok(Self {
            root_id: id.clone(),
            id,
            generation: 0,
            package,
            assets,
            runtime,
            store,
            commands: BTreeMap::new(),
            viewers: BTreeMap::new(),
            interactions: BTreeMap::new(),
        })
    }

    pub(crate) fn open(
        id: impl Into<String>,
        package: PackageIdentity,
        assets: BTreeSet<String>,
        world: World,
        seed: u64,
        modules: BTreeMap<String, String>,
        entry: &str,
        request: Value,
    ) -> Result<Self, String> {
        let mut session = Self::load_entry(id, package, assets, world, seed, modules, entry)?;
        session.initialize(request)?;
        Ok(session)
    }

    pub(crate) fn restore_entry(
        id: impl Into<String>,
        package: PackageIdentity,
        assets: BTreeSet<String>,
        world: World,
        seed: u64,
        modules: BTreeMap<String, String>,
        entry: &str,
        bytes: &[u8],
    ) -> Result<Self, String> {
        let mut session = Self::load_entry(id, package, assets, world, seed, modules, entry)?;
        session.restore(bytes).map_err(|error| error.message)?;
        Ok(session)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn world(&self) -> &World {
        self.store.world()
    }

    pub fn world_revision(&self) -> u64 {
        self.store.revision()
    }

    pub fn save(&self) -> Result<Vec<u8>, Error> {
        persistence::encode(self.package.clone(), self.store.snapshot())
    }

    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let snapshot = persistence::decode(bytes, &self.package)?;
        self.restore_store(snapshot)
    }

    fn initialize(&mut self, request: Value) -> Result<Value, String> {
        let transaction = self.store.begin(self.store.revision())?;
        let (value, transaction) = self
            .runtime
            .initialize(transaction, request)
            .map_err(|error| error.to_string())?;
        self.store.commit(transaction)?;
        Ok(value)
    }

    pub fn snapshot(&mut self, viewer: &str) -> Result<ViewSnapshot, String> {
        if viewer.is_empty() {
            return Err("viewer id must not be empty".into());
        }
        let context = self
            .viewers
            .get(viewer)
            .and_then(|state| state.context.clone())
            .unwrap_or_else(|| Value::Map(BTreeMap::new()));
        let request = Value::Map(BTreeMap::from([
            ("viewer".into(), Value::String(viewer.into())),
            ("view_context".into(), context),
            ("events".into(), Value::List(Vec::new())),
            ("command_id".into(), Value::String("snapshot".into())),
        ]));
        let transaction = self.store.begin(self.store.revision())?;
        let presentation = parse_presentation(
            self.runtime
                .present(transaction, request)
                .map_err(|error| error.to_string())?,
        )?;
        let state = self.viewers.entry(viewer.into()).or_default();
        let mut blocks = state.blocks.clone();
        for id in presentation.remove_ids {
            blocks.remove(&id);
        }
        for block in presentation.replace_blocks {
            blocks.insert(block.id, block.content);
        }
        if blocks != state.blocks {
            state.revision = state
                .revision
                .checked_add(1)
                .ok_or_else(|| "view revision exhausted".to_owned())?;
            state.blocks = blocks;
        }
        let snapshot = ViewSnapshot {
            world_revision: self.store.revision(),
            view_revision: state.revision,
            blocks: state
                .blocks
                .iter()
                .map(|(id, content)| ViewBlock {
                    id: id.clone(),
                    content: content.clone(),
                })
                .collect(),
        };
        protocol::validate_assets(&Message::ViewSnapshot(snapshot.clone()), &self.assets)
            .map_err(|error| error.message)?;
        Ok(snapshot)
    }

    pub fn dispatch(&mut self, viewer: &str, command: Command) -> Dispatch {
        if viewer.is_empty() {
            return rejected(
                &command,
                self.store.revision(),
                "invalid_input",
                "viewer id must not be empty",
            );
        }
        if let Some(cached) = self.commands.get(&command.command_id) {
            return if cached.viewer == viewer && cached.command == command {
                cached.dispatch.clone()
            } else {
                rejected(
                    &command,
                    self.store.revision(),
                    "conflict",
                    "command id was already used with different content",
                )
            };
        }

        if let Err(error) = protocol::validate(&Message::Command(command.clone())) {
            return Dispatch {
                result: CommandResult {
                    command_id: command.command_id,
                    status: CommandStatus::Rejected,
                    world_revision: self.store.revision(),
                    error: Some(error),
                },
                events: None,
                changes: None,
                view: None,
                presentation_error: None,
            };
        }

        let mut dispatch = if command.expected_world_revision != self.store.revision() {
            rejected(
                &command,
                self.store.revision(),
                "conflict",
                "stale world revision",
            )
        } else {
            let executed = match self.store.begin(command.expected_world_revision) {
                Err(message) => Err(Error::new("conflict", message)),
                Ok(transaction) => {
                    match self.runtime.dispatch(transaction, command_value(&command)) {
                        Err(error) => Err(runtime_error(error)),
                        Ok((events, transaction)) => self
                            .store
                            .commit(transaction)
                            .map(|commit| (events, commit))
                            .map_err(|message| Error::new("conflict", message)),
                    }
                }
            };
            match executed {
                Ok((events, commit)) => Dispatch {
                    result: CommandResult {
                        command_id: command.command_id.clone(),
                        status: CommandStatus::Committed,
                        world_revision: commit.revision,
                        error: None,
                    },
                    events: Some(events),
                    changes: Some(commit),
                    view: None,
                    presentation_error: None,
                },
                Err(error) => rejected_with_error(&command, self.store.revision(), error),
            }
        };
        if dispatch.result.status == CommandStatus::Committed {
            match self.update_view(
                viewer,
                &command.command_id,
                dispatch.events.clone().unwrap_or(Value::Null),
            ) {
                Ok(view) => dispatch.view = view,
                Err(error) => dispatch.presentation_error = Some(error),
            }
        }
        self.commands.insert(
            command.command_id.clone(),
            CachedCommand {
                viewer: viewer.into(),
                command,
                dispatch: dispatch.clone(),
            },
        );
        dispatch
    }

    pub fn interact(&mut self, viewer: &str, interaction: Interaction) -> InteractionDelivery {
        if viewer.is_empty() {
            return interaction_error(
                &interaction.interaction_id,
                "invalid_input",
                "viewer id must not be empty",
            );
        }
        if let Some(cached) = self.interactions.get(&interaction.interaction_id) {
            return if cached.viewer == viewer && cached.interaction == interaction {
                cached.delivery.clone()
            } else {
                interaction_error(
                    &interaction.interaction_id,
                    "conflict",
                    "interaction id was already used with different content",
                )
            };
        }
        let current_view = self.viewers.get(viewer).map_or(0, |state| state.revision);
        let delivery = if interaction.expected_view_revision != current_view {
            interaction_error(
                &interaction.interaction_id,
                "conflict",
                "stale view revision",
            )
        } else {
            self.interact_once(viewer, &interaction)
                .unwrap_or_else(|error| interaction_failure(&interaction.interaction_id, error))
        };
        self.interactions.insert(
            interaction.interaction_id.clone(),
            CachedInteraction {
                viewer: viewer.into(),
                interaction,
                delivery: delivery.clone(),
            },
        );
        delivery
    }

    fn interact_once(
        &mut self,
        viewer: &str,
        interaction: &Interaction,
    ) -> Result<InteractionDelivery, Error> {
        let context = self
            .viewers
            .get(viewer)
            .and_then(|state| state.context.clone())
            .unwrap_or_else(|| Value::Map(BTreeMap::new()));
        let input = Value::Map(BTreeMap::from([
            (
                "interaction_id".into(),
                Value::String(interaction.interaction_id.clone()),
            ),
            (
                "kind".into(),
                Value::String(
                    match interaction.kind {
                        InteractionKind::Activate => "activate",
                        InteractionKind::CellClick => "cell_click",
                        InteractionKind::Hover => "hover",
                        InteractionKind::Dismiss => "dismiss",
                    }
                    .into(),
                ),
            ),
            ("target".into(), interaction.target.clone()),
            ("payload".into(), interaction.payload.clone()),
        ]));
        let request = Value::Map(BTreeMap::from([
            ("viewer".into(), Value::String(viewer.into())),
            ("view_context".into(), context),
            ("input".into(), input),
        ]));
        let transaction = self
            .store
            .begin(self.store.revision())
            .map_err(|message| Error::new("conflict", message))?;
        let result = parse_interaction_result(
            self.runtime
                .interact(transaction, request)
                .map_err(runtime_error)?,
        )
        .map_err(|message| Error::new("script_error", message))?;
        self.viewers.entry(viewer.into()).or_default().context = Some(result.0);
        let command = result.1.map(|action| {
            self.dispatch(
                viewer,
                Command {
                    command_id: format!("interaction:{}", interaction.interaction_id),
                    expected_world_revision: self.store.revision(),
                    action: action.action,
                    payload: action.payload,
                },
            )
        });
        let view = if command.is_none() {
            self.update_view(
                viewer,
                &format!("interaction:{}", interaction.interaction_id),
                Value::List(Vec::new()),
            )?
        } else {
            None
        };
        Ok(InteractionDelivery {
            interaction_id: interaction.interaction_id.clone(),
            view,
            command,
            error: None,
        })
    }

    fn update_view(
        &mut self,
        viewer: &str,
        command_id: &str,
        events: Value,
    ) -> Result<Option<ViewUpdate>, Error> {
        let context = self
            .viewers
            .get(viewer)
            .and_then(|state| state.context.clone())
            .unwrap_or_else(|| Value::Map(BTreeMap::new()));
        let request = Value::Map(BTreeMap::from([
            ("viewer".into(), Value::String(viewer.into())),
            ("view_context".into(), context),
            (
                "events".into(),
                match events {
                    Value::List(events) => Value::List(events),
                    event => Value::List(vec![event]),
                },
            ),
            ("command_id".into(), Value::String(command_id.into())),
        ]));
        let transaction = self
            .store
            .begin(self.store.revision())
            .map_err(|message| Error::new("conflict", message))?;
        let presentation = parse_presentation(
            self.runtime
                .present(transaction, request)
                .map_err(runtime_error)?,
        )
        .map_err(|message| Error::new("script_error", message))?;
        let state = self.viewers.entry(viewer.into()).or_default();
        let base = state.revision;
        let mut blocks = state.blocks.clone();
        for id in &presentation.remove_ids {
            blocks.remove(id);
        }
        for block in &presentation.replace_blocks {
            blocks.insert(block.id.clone(), block.content.clone());
        }
        let changed = blocks != state.blocks || !presentation.effects.is_empty();
        if !changed {
            return Ok(None);
        }
        let revision = base
            .checked_add(1)
            .ok_or_else(|| Error::new("conflict", "view revision exhausted"))?;
        let update = ViewUpdate {
            world_revision: self.store.revision(),
            base_view_revision: base,
            view_revision: revision,
            replace_blocks: presentation.replace_blocks,
            remove_ids: presentation.remove_ids,
            effects: presentation.effects,
        };
        protocol::validate_assets(&Message::ViewUpdate(update.clone()), &self.assets)?;
        state.blocks = blocks;
        state.revision = revision;
        Ok(Some(update))
    }

    fn restore_store(&mut self, snapshot: StoreSnapshot) -> Result<(), Error> {
        let store = Store::from_snapshot(snapshot)
            .map_err(|message| Error::new("invalid_input", message))?;
        let transaction = store
            .begin(store.revision())
            .map_err(|message| Error::new("conflict", message))?;
        self.runtime
            .validate_restored(transaction)
            .map_err(runtime_error)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| Error::new("conflict", "session generation exhausted"))?;
        self.store = store;
        self.generation = generation;
        self.id = format!("{}@{generation}", self.root_id);
        self.commands.clear();
        self.viewers.clear();
        self.interactions.clear();
        Ok(())
    }
}

fn parse_presentation(value: Value) -> Result<Presentation, String> {
    let Value::Map(mut value) = value else {
        return Err("presentation must be a record".into());
    };
    Ok(Presentation {
        replace_blocks: parse_items(value.remove("replace_blocks"), |mut item| {
            let id = take_string(&mut item, "id")?;
            let content = item
                .remove("content")
                .ok_or_else(|| "view block has no content".to_owned())?;
            Ok(ViewBlock { id, content })
        })?,
        remove_ids: match value.remove("remove_ids") {
            Some(Value::List(values)) => values
                .into_iter()
                .map(|value| match value {
                    Value::String(value) => Ok(value),
                    _ => Err("removed block id must be a string".into()),
                })
                .collect::<Result<_, String>>()?,
            _ => return Err("presentation remove_ids must be a list".into()),
        },
        effects: parse_items(value.remove("effects"), |mut item| {
            let id = take_string(&mut item, "id")?;
            let content = item
                .remove("content")
                .ok_or_else(|| "effect has no content".to_owned())?;
            Ok(Effect { id, content })
        })?,
    })
}

fn parse_interaction_result(value: Value) -> Result<(Value, Option<Action>), String> {
    let Value::Map(mut value) = value else {
        return Err("interaction result must be a record".into());
    };
    let context = value
        .remove("view_context")
        .ok_or_else(|| "interaction result has no view_context".to_owned())?;
    let command = match value.remove("command") {
        None | Some(Value::Null) => None,
        Some(Value::Map(mut command)) => Some(Action {
            action: take_string(&mut command, "action")?,
            payload: command.remove("payload").unwrap_or(Value::Null),
        }),
        Some(_) => return Err("interaction command must be a record".into()),
    };
    Ok((context, command))
}

fn parse_items<T>(
    value: Option<Value>,
    parse: impl Fn(BTreeMap<String, Value>) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    let Some(Value::List(values)) = value else {
        return Err("presentation field must be a list".into());
    };
    values
        .into_iter()
        .map(|value| match value {
            Value::Map(value) => parse(value),
            _ => Err("presentation item must be a record".into()),
        })
        .collect()
}

fn take_string(value: &mut BTreeMap<String, Value>, field: &str) -> Result<String, String> {
    match value.remove(field) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value),
        _ => Err(format!("presentation {field} must be a nonempty string")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::store::Map;

    fn command(id: &str, revision: u64, payload: Value) -> Command {
        Command {
            command_id: id.into(),
            expected_world_revision: revision,
            action: "custom".into(),
            payload,
        }
    }

    #[test]
    fn retries_are_deduplicated_before_revision_checks() {
        assert_eq!(
            runtime_error(RuntimeError::BudgetExceeded).code,
            "budget_exceeded"
        );
        let mut session = Session::load_entry(
            "test",
            PackageIdentity {
                package_id: "test".into(),
                package_version: "1".into(),
                protocol_version: 1,
            },
            BTreeSet::new(),
            World {
                map: Map {
                    width: 1,
                    height: 1,
                    cells: vec!["Gg".into()],
                },
                objects: BTreeMap::new(),
                state: BTreeMap::new(),
            },
            1,
            BTreeMap::from([(
                "game.init".into(),
                "return {
                    dispatch=function(c) local n=(c.state:get('count') or 0)+1 c.state:set('count',n) return n end,
                    present=function(_,r) return {
                        replace_blocks={{id='viewer',content=r.viewer}},
                        remove_ids=wesnoth.value.list(), effects=wesnoth.value.list()
                    } end,
                    interact=function(_,r) return {
                        view_context=r.view_context,
                        command={action='custom',payload=wesnoth.value.record()}
                    } end,
                    validate_restored=function(c)
                        assert(c.state:get('count') ~= 99, 'invalid count')
                        return true
                    end
                }".into(),
            )]),
            "game.init",
        )
        .unwrap();
        let request = command("c1", 0, Value::Null);
        let first = session.dispatch("viewer", request.clone());
        assert_eq!(first.result.status, CommandStatus::Committed);
        assert_eq!(first.events, Some(Value::Integer(1)));
        assert_eq!(
            session.dispatch("viewer", request.clone()).events,
            first.events
        );
        assert_eq!(
            session
                .dispatch("intruder", request)
                .result
                .error
                .unwrap()
                .code,
            "conflict"
        );
        assert_eq!(
            session
                .dispatch("", command("empty", 1, Value::Null))
                .result
                .error
                .unwrap()
                .code,
            "invalid_input"
        );
        assert_eq!(session.world().state["count"], Value::Integer(1));

        let conflict = session.dispatch("viewer", command("c1", 0, Value::Bool(true)));
        assert_eq!(conflict.result.status, CommandStatus::Rejected);
        let stale = session.dispatch("viewer", command("c2", 0, Value::Null));
        assert_eq!(stale.result.status, CommandStatus::Rejected);

        let first_view = session.snapshot("alice").unwrap();
        let same_view = session.snapshot("alice").unwrap();
        let other_view = session.snapshot("bob").unwrap();
        assert_eq!(first_view.view_revision, 1);
        assert_eq!(same_view.view_revision, 1);
        assert_eq!(other_view.view_revision, 1);
        assert_eq!(first_view.blocks[0].content, Value::String("alice".into()));

        let interaction = Interaction {
            interaction_id: "i1".into(),
            expected_view_revision: 1,
            kind: InteractionKind::Activate,
            target: Value::String("viewer".into()),
            payload: Value::Null,
        };
        let delivery = session.interact("alice", interaction.clone());
        assert!(delivery.error.is_none());
        assert_eq!(
            delivery.command.as_ref().unwrap().events,
            Some(Value::Integer(2))
        );
        assert_eq!(
            session
                .interact("bob", interaction.clone())
                .error
                .unwrap()
                .code,
            "conflict"
        );
        assert_eq!(
            session.interact("alice", interaction).command,
            delivery.command
        );
        assert_eq!(session.world().state["count"], Value::Integer(2));

        let saved = session.save().unwrap();
        let original_id = session.id().to_owned();
        session.dispatch("viewer", command("c3", 2, Value::Null));
        assert_eq!(session.world().state["count"], Value::Integer(3));
        session.restore(&saved).unwrap();
        assert_ne!(session.id(), original_id);
        let restored_id = session.id().to_owned();
        assert_eq!(session.world().state["count"], Value::Integer(2));
        let mut invalid: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        invalid["store"]["world"]["state"]["count"] = 99.into();
        assert!(
            session
                .restore(&serde_json::to_vec(&invalid).unwrap())
                .is_err()
        );
        assert_eq!(session.id(), restored_id);
        assert_eq!(session.world().state["count"], Value::Integer(2));
    }
}

fn command_value(command: &Command) -> Value {
    Value::Map(BTreeMap::from([
        ("action".into(), Value::String(command.action.clone())),
        ("payload".into(), command.payload.clone()),
    ]))
}

fn rejected(command: &Command, revision: u64, code: &str, message: impl Into<String>) -> Dispatch {
    rejected_with_error(command, revision, Error::new(code, message))
}

fn rejected_with_error(command: &Command, revision: u64, error: Error) -> Dispatch {
    Dispatch {
        result: CommandResult {
            command_id: command.command_id.clone(),
            status: CommandStatus::Rejected,
            world_revision: revision,
            error: Some(error),
        },
        events: None,
        changes: None,
        view: None,
        presentation_error: None,
    }
}

fn interaction_error(
    interaction_id: &str,
    code: &str,
    message: impl Into<String>,
) -> InteractionDelivery {
    interaction_failure(interaction_id, Error::new(code, message))
}

fn interaction_failure(interaction_id: &str, error: Error) -> InteractionDelivery {
    InteractionDelivery {
        interaction_id: interaction_id.into(),
        view: None,
        command: None,
        error: Some(error),
    }
}

fn runtime_error(error: RuntimeError) -> Error {
    Error::new(error.code(), error.to_string())
}
