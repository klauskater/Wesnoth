//! Минимальное изменяемое ядро игрового движка.
//!
//! У ядра две обязанности:
//!
//! - хранить единственное истинное состояние игрового мира;
//! - транзакционно запускать Lua-обработчик события игрока или тика времени.
//!
//! Здесь нет игроков, окон, элементов интерфейса, представлений мира и сетевого
//! протокола. Эти слои могут проверять свои идентификаторы и версии снаружи, а
//! затем передавать сюда только смысловое событие и ожидаемую ревизию мира.

use std::collections::BTreeMap;

use super::{
    runtime::{Runtime, RuntimeError},
    store::{Commit, Store, StoreSnapshot, World},
    value::Value,
};

/// Результат одного успешно выполненного изменяющего скрипта.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineChange {
    /// Значение, которое вернул сценарный обработчик.
    pub output: Value,
    /// Новая ревизия мира и точный список изменённых адресов хранилища.
    pub commit: Commit,
}

/// Ошибка на границе хранилища или сценарного обработчика.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
}

impl EngineError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Авторитетное состояние мира и среда выполнения его правил.
pub struct Engine {
    runtime: Runtime,
    store: Store,
}

impl Engine {
    /// Создаёт ещё не инициализированный движок из уже загруженных ресурсов.
    pub(crate) fn load(
        world: World,
        seed: u64,
        modules: BTreeMap<String, String>,
        entry: &str,
    ) -> Result<Self, String> {
        let (runtime, store) = Runtime::load_entry(world, seed, modules, entry)?;
        Ok(Self { runtime, store })
    }

    /// Один раз запускает сценарный обработчик начального состояния.
    pub(crate) fn initialize(&mut self, request: Value) -> Result<Value, EngineError> {
        self.run("initialize", request).map(|change| change.output)
    }

    /// Передаёт движку смысловое изменение, вызванное игроком.
    pub fn user_event(
        &mut self,
        expected_revision: u64,
        event: Value,
    ) -> Result<EngineChange, EngineError> {
        self.run_at(expected_revision, "dispatch", event)
    }

    /// Передаёт движку один тик игрового времени.
    ///
    /// Планирование тиков находится снаружи. Движок получает только прошедшее
    /// время и обрабатывает его так же атомарно, как действие игрока.
    pub fn tick(
        &mut self,
        expected_revision: u64,
        elapsed_ms: u64,
    ) -> Result<EngineChange, EngineError> {
        let elapsed_ms = i64::try_from(elapsed_ms)
            .map_err(|_| EngineError::new("invalid_input", "tick duration is too large"))?;
        self.run_at(
            expected_revision,
            "tick",
            Value::Map(BTreeMap::from([(
                "elapsed_ms".into(),
                Value::Integer(elapsed_ms),
            )])),
        )
    }

    pub fn revision(&self) -> u64 {
        self.store.revision()
    }

    pub fn world(&self) -> &World {
        self.store.world()
    }

    pub(crate) fn snapshot(&self) -> StoreSnapshot {
        self.store.snapshot()
    }

    /// Временная граница для старого слоя представления в `Session`.
    /// Удаляется вместе с переносом presentation-скриптов в модуль интерфейса.
    pub(crate) fn read_script(&self, handler: &str, request: Value) -> Result<Value, EngineError> {
        let transaction = self
            .store
            .begin(self.store.revision())
            .map_err(|message| EngineError::new("conflict", message))?;
        self.runtime
            .read(transaction, handler, request)
            .map_err(EngineError::from)
    }

    pub(crate) fn restore(&mut self, snapshot: StoreSnapshot) -> Result<(), EngineError> {
        let store = Store::from_snapshot(snapshot)
            .map_err(|message| EngineError::new("invalid_input", message))?;
        let transaction = store
            .begin(store.revision())
            .map_err(|message| EngineError::new("conflict", message))?;
        self.runtime
            .read(transaction, "validate_restored", Value::Nil)
            .map_err(EngineError::from)?;
        self.store = store;
        Ok(())
    }

    fn run(&mut self, handler: &str, input: Value) -> Result<EngineChange, EngineError> {
        self.run_at(self.store.revision(), handler, input)
    }

    fn run_at(
        &mut self,
        expected_revision: u64,
        handler: &str,
        input: Value,
    ) -> Result<EngineChange, EngineError> {
        let transaction = self
            .store
            .begin(expected_revision)
            .map_err(|message| EngineError::new("conflict", message))?;
        let (output, transaction) = self
            .runtime
            .write(transaction, handler, input)
            .map_err(EngineError::from)?;
        let commit = self
            .store
            .commit(transaction)
            .map_err(|message| EngineError::new("conflict", message))?;
        Ok(EngineChange { output, commit })
    }
}

impl From<RuntimeError> for EngineError {
    fn from(error: RuntimeError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::store::Map;

    fn engine() -> Engine {
        Engine::load(
            World {
                map: Map {
                    width: 1,
                    height: 1,
                    cells: vec![Value::Map(BTreeMap::new())],
                },
                entities: BTreeMap::new(),
                data: BTreeMap::new(),
            },
            1,
            BTreeMap::from([(
                "game.init".into(),
                "return {
                    initialize=function(c) c.data:set('value', 0) return true end,
                    dispatch=function(c, e)
                        c.data:set('value', c.data:get('value') + e.amount)
                        return e.amount
                    end,
                    tick=function(c, t)
                        c.data:set('value', c.data:get('value') + t.elapsed_ms)
                        return t.elapsed_ms
                    end,
                    validate_restored=function() return true end
                }"
                .into(),
            )]),
            "game.init",
        )
        .unwrap()
    }

    #[test]
    fn user_events_and_ticks_are_the_only_mutating_inputs() {
        let mut engine = engine();
        engine.initialize(Value::Nil).unwrap();
        let event = engine
            .user_event(
                engine.revision(),
                Value::Map(BTreeMap::from([("amount".into(), Value::Integer(2))])),
            )
            .unwrap();
        assert_eq!(event.output, Value::Integer(2));
        let tick = engine.tick(engine.revision(), 15).unwrap();
        assert_eq!(tick.output, Value::Integer(15));
        assert_eq!(engine.world().data["value"], Value::Integer(17));
    }

    #[test]
    fn failed_or_stale_changes_do_not_modify_the_world() {
        let mut engine = engine();
        engine.initialize(Value::Nil).unwrap();
        let revision = engine.revision();
        assert!(engine.tick(revision + 1, 10).is_err());
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.world().data["value"], Value::Integer(0));
    }
}
