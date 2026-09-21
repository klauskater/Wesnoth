//! Протокольный адаптер поверх минимального [`Engine`](super::Engine).
//!
//! Этот модуль пока сохраняет доставку команд, состояние наблюдателей и старый
//! протокол представления. Авторитетным миром и выполнением изменяющих скриптов
//! он больше не владеет: эти обязанности находятся в `Engine`.
//!
//! Место `Session` в цепочке вызовов:
//!
//! ```text
//! клиент/UI
//!    │ Command / Interaction / запрос снимка
//!    ▼
//! Session ── проверка протокола, ревизий и повторных запросов
//!    │
//!    ├── Engine::user_event(...) ── изменяет игровой мир через Lua
//!    │
//!    └── Engine::read_script(...) ── только читает мир и строит представление
//! ```
//!
//! Поэтому здесь существуют две независимые ревизии:
//!
//! - `world_revision` принадлежит движку и меняется после успешной игровой
//!   команды;
//! - `view_revision` принадлежит конкретному наблюдателю и меняется, когда его
//!   набор блоков интерфейса или эффектов действительно обновился.
//!
//! Сессия также делает повторную доставку безопасной. Если транспорт дважды
//! прислал одну команду с тем же идентификатором, второй раз Lua не запускается:
//! клиент получает сохранённый результат первого выполнения.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    Engine, EngineError,
    persistence,
    protocol::{
        self, Action, Command, CommandResult, CommandStatus, Effect, Error, Interaction,
        InteractionKind, Message, Presentation, ViewBlock, ViewSnapshot, ViewUpdate,
    },
    resources::PackageIdentity,
    store::{Commit, StoreSnapshot, World},
    value::Value,
};

#[derive(Clone, Debug, PartialEq)]
/// Полный результат доставки одной смысловой игровой команды.
///
/// Команда и построение представления разделены намеренно. Игровое изменение
/// может успешно попасть в мир, даже если последующий скрипт представления
/// завершился ошибкой. В этом случае `result` остаётся `Committed`, изменения
/// нельзя повторять, а ошибка возвращается отдельно в `presentation_error`.
pub struct Dispatch {
    /// Подтверждение либо отклонение команды для транспортного протокола.
    pub result: CommandResult,
    /// Смысловые события, которые вернул игровой Lua-скрипт.
    pub events: Option<Value>,
    /// Зафиксированные движком изменения мира и их новая ревизия.
    pub changes: Option<Commit>,
    /// Инкрементальное изменение представления именно этого наблюдателя.
    pub view: Option<ViewUpdate>,
    /// Ошибка построения представления после уже совершённой команды.
    pub presentation_error: Option<Error>,
}

#[derive(Clone)]
/// Запись для идемпотентной повторной доставки команды.
///
/// Одного `command_id` недостаточно: при повторе проверяются также наблюдатель и
/// всё содержимое команды. Повтор с другим содержимым считается конфликтом.
struct CachedCommand {
    viewer: String,
    command: Command,
    dispatch: Dispatch,
}

#[derive(Clone, Debug, PartialEq)]
/// Результат обработки локального действия интерфейса.
///
/// Взаимодействие может изменить только представление (`view`) либо породить
/// настоящую игровую команду (`command`). Само по себе нажатие на ячейку или
/// кнопку не получает права напрямую изменять мир.
pub struct InteractionDelivery {
    pub interaction_id: String,
    pub view: Option<ViewUpdate>,
    pub command: Option<Dispatch>,
    pub error: Option<Error>,
}

#[derive(Clone)]
/// Кэш уже обработанного UI-взаимодействия, аналогичный `CachedCommand`.
struct CachedInteraction {
    viewer: String,
    interaction: Interaction,
    delivery: InteractionDelivery,
}

#[derive(Default)]
/// Персональное состояние представления одного наблюдателя.
///
/// Оно не является частью игрового мира и не сохраняется в save-файл. Здесь
/// допустимы выбранная клетка, открытая панель и другие локальные детали UI.
struct Viewer {
    /// Версия последнего представления, которое получил этот наблюдатель.
    revision: u64,
    /// Непрозрачная для Rust память Lua-слоя взаимодействия.
    context: Option<Value>,
    /// Последнее полное содержимое блоков представления по их стабильным ID.
    blocks: BTreeMap<String, Value>,
}

/// Протокольная оболочка одной запущенной игры.
///
/// `Session` владеет движком, но не реализует игровые правила. Она отвечает за
/// жизненный цикл экземпляра, согласование ревизий, дедупликацию доставки и
/// персональные представления клиентов.
pub struct Session {
    /// Текущий ID экземпляра. После восстановления получает суффикс поколения.
    id: String,
    /// Исходный ID, относительно которого формируются ID новых поколений.
    root_id: String,
    /// Сколько успешных восстановлений состояния пережил этот объект сессии.
    generation: u64,
    /// Идентичность пакета нужна для проверки совместимости сохранений.
    package: PackageIdentity,
    /// Разрешённые пакетом ID ресурсов для проверки ответов представления.
    assets: BTreeSet<String>,
    /// Единственный владелец авторитетного игрового мира и Lua runtime.
    engine: Engine,
    /// Завершённые команды по `command_id` для защиты от повторного исполнения.
    commands: BTreeMap<String, CachedCommand>,
    /// Независимое состояние представления каждого подключённого наблюдателя.
    viewers: BTreeMap<String, Viewer>,
    /// Завершённые UI-взаимодействия по `interaction_id`.
    interactions: BTreeMap<String, CachedInteraction>,
}

/// Уже загруженные ресурсы, из которых можно создать сессию.
///
/// Загрузчик ресурсов формирует эту структуру до запуска игры. Благодаря этому
/// `Session` не читает файлы и не решает, какое приключение или правила выбрать.
pub(crate) struct SessionInput {
    /// Стабильный ID новой сессии, обычно пакет плюс путь процесса главы.
    pub id: String,
    /// Версия пакета и протокола, с которыми создан мир.
    pub package: PackageIdentity,
    /// Белый список доступных представлению ресурсов.
    pub assets: BTreeSet<String>,
    /// Начальная карта и пустые либо восстановленные хранилища мира.
    pub world: World,
    /// Начальное состояние детерминированного генератора случайных чисел.
    pub seed: u64,
    /// Полный набор уже прочитанных Lua-модулей пакета.
    pub modules: BTreeMap<String, String>,
    /// Имя корневого Lua-модуля с `initialize`, `dispatch`, `present` и т. д.
    pub entry: String,
}

impl Session {
    /// Создаёт оболочку сессии и загружает Lua entrypoint, но ещё не запускает
    /// инициализацию новой игры и не восстанавливает сохранение.
    ///
    /// Это общий нижний этап для [`Session::open`] и
    /// [`Session::restore_entry`]. На выходе движок уже способен выполнять
    /// скрипты, однако переданный мир ещё не считается начатой игровой сессией.
    fn load_entry(input: SessionInput) -> Result<Self, String> {
        let SessionInput {
            id,
            package,
            assets,
            world,
            seed,
            modules,
            entry,
        } = input;
        // ID участвует в дедупликации и именовании поколений, поэтому без него
        // невозможно однозначно идентифицировать экземпляр игры.
        if id.is_empty() {
            return Err("session id must not be empty".into());
        }
        // Lua-пакет и Rust-клиент обязаны одинаково понимать структуры Command,
        // ViewUpdate и остальные сообщения протокола.
        if package.protocol_version != protocol::PROTOCOL_VERSION {
            return Err(format!(
                "incompatible package protocol: {} (expected {})",
                package.protocol_version,
                protocol::PROTOCOL_VERSION
            ));
        }
        // Здесь создаётся Runtime и загружается корневой Lua-модуль. Ни один
        // изменяющий игровой обработчик на этом шаге ещё не вызывается.
        let engine = Engine::load(world, seed, modules, &entry)?;
        Ok(Self {
            root_id: id.clone(),
            id,
            generation: 0,
            package,
            assets,
            engine,
            commands: BTreeMap::new(),
            viewers: BTreeMap::new(),
            interactions: BTreeMap::new(),
        })
    }

    /// Открывает новую игру.
    ///
    /// Сначала создаётся пустая сессия с загруженным Lua-кодом, затем `request`
    /// один раз передаётся в `entry.initialize`. Именно инициализация наполняет
    /// хранилища сущностями и общеигровыми данными.
    pub(crate) fn open(input: SessionInput, request: Value) -> Result<Self, String> {
        let mut session = Self::load_entry(input)?;
        session.initialize(request)?;
        Ok(session)
    }

    /// Создаёт runtime из актуальных ресурсов и помещает в него сохранённый мир.
    ///
    /// Lua-код никогда не берётся из save-файла: он загружается из `input`, а
    /// сохранение содержит только данные мира, RNG и ревизию. После установки
    /// снимка движок вызывает проверку восстановленного состояния.
    pub(crate) fn restore_entry(input: SessionInput, bytes: &[u8]) -> Result<Self, String> {
        let mut session = Self::load_entry(input)?;
        session.restore(bytes).map_err(|error| error.message)?;
        Ok(session)
    }

    /// Возвращает ID именно текущего поколения сессии.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Даёт внутреннему игровому фасаду доступ к миру только для чтения.
    pub(crate) fn world(&self) -> &World {
        self.engine.world()
    }

    /// Текущая авторитетная ревизия игрового мира.
    pub fn world_revision(&self) -> u64 {
        self.engine.revision()
    }

    /// Кодирует авторитетный снимок движка вместе с идентичностью пакета.
    /// Локальные состояния UI и кэши доставки намеренно не сохраняются.
    pub fn save(&self) -> Result<Vec<u8>, Error> {
        persistence::encode(self.package.clone(), self.engine.snapshot())
    }

    /// Восстанавливает состояние в уже существующий объект сессии.
    /// Совместимость пакета проверяется до изменения текущего мира.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let snapshot = persistence::decode(bytes, &self.package)?;
        self.restore_store(snapshot)
    }

    /// Единственный вызов Lua-инициализации новой игры со стороны сессии.
    fn initialize(&mut self, request: Value) -> Result<Value, String> {
        self.engine
            .initialize(request)
            .map_err(|error| error.message)
    }

    /// Строит полный снимок представления для одного наблюдателя.
    ///
    /// `snapshot` не изменяет игровой мир. Lua-функция `present` читает мир и
    /// возвращает операции над именованными блоками. Сессия применяет их к ранее
    /// известному состоянию наблюдателя и отдаёт уже собранный полный набор.
    pub fn snapshot(&mut self, viewer: &str) -> Result<ViewSnapshot, String> {
        if viewer.is_empty() {
            return Err("viewer id must not be empty".into());
        }
        // Новый наблюдатель начинает с пустого UI-контекста. Для существующего
        // передаём контекст, который ранее вернул скрипт `interact`.
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
        // `read_script` гарантирует режим только для чтения: `present` не может
        // легально изменить карту, сущности, общие данные или RNG.
        let presentation = parse_presentation(
            self.engine
                .read_script("present", request)
                .map_err(|error| error.message)?,
        )?;
        let state = self.viewers.entry(viewer.into()).or_default();
        // Lua возвращает патч, но полный снимок требует применить этот патч к
        // последнему известному набору блоков наблюдателя.
        let mut blocks = state.blocks.clone();
        for id in presentation.remove_ids {
            blocks.remove(&id);
        }
        for block in presentation.replace_blocks {
            blocks.insert(block.id, block.content);
        }
        // Повторный снимок с тем же содержимым не увеличивает view_revision.
        if blocks != state.blocks {
            state.revision = state
                .revision
                .checked_add(1)
                .ok_or_else(|| "view revision exhausted".to_owned())?;
            state.blocks = blocks;
        }
        let snapshot = ViewSnapshot {
            world_revision: self.engine.revision(),
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
        // Скрипт представления не может сослаться на ресурс, которого не было в
        // подготовленном пакете сессии.
        protocol::validate_assets(&Message::ViewSnapshot(snapshot.clone()), &self.assets)
            .map_err(|error| error.message)?;
        Ok(snapshot)
    }

    /// Проверяет и выполняет одну смысловую игровую команду.
    ///
    /// Порядок проверок принципиален: сначала обрабатывается точный повтор уже
    /// выполненной команды, затем проверяется форма сообщения и только потом —
    /// ожидаемая ревизия мира. Поэтому потерянный сетевой ответ можно запросить
    /// повторно даже после того, как первая доставка уже увеличила ревизию.
    pub fn dispatch(&mut self, viewer: &str, command: Command) -> Dispatch {
        if viewer.is_empty() {
            return rejected(
                &command,
                self.engine.revision(),
                "invalid_input",
                "viewer id must not be empty",
            );
        }
        // Идемпотентный повтор возвращает прежний ответ. Повторное использование
        // ID другим клиентом или с другим телом команды является конфликтом.
        if let Some(cached) = self.commands.get(&command.command_id) {
            return if cached.viewer == viewer && cached.command == command {
                cached.dispatch.clone()
            } else {
                rejected(
                    &command,
                    self.engine.revision(),
                    "conflict",
                    "command id was already used with different content",
                )
            };
        }

        // Здесь проверяются обязательные поля и общие ограничения протокола до
        // того, как недоверенные данные попадут в Lua.
        if let Err(error) = protocol::validate(&Message::Command(command.clone())) {
            return Dispatch {
                result: CommandResult {
                    command_id: command.command_id,
                    status: CommandStatus::Rejected,
                    world_revision: self.engine.revision(),
                    error: Some(error),
                },
                events: None,
                changes: None,
                view: None,
                presentation_error: None,
            };
        }

        // Оптимистическая блокировка защищает от команды, рассчитанной на уже
        // устаревшее состояние мира.
        let mut dispatch = if command.expected_world_revision != self.engine.revision() {
            rejected(
                &command,
                self.engine.revision(),
                "conflict",
                "stale world revision",
            )
        } else {
            // Только эта ветка способна изменить мир. Engine выполняет Lua
            // транзакционно и возвращает как вывод скрипта, так и Commit.
            let executed = self
                .engine
                .user_event(command.expected_world_revision, command_value(&command))
                .map(|change| (change.output, change.commit))
                .map_err(engine_error);
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
                Err(error) => rejected_with_error(&command, self.engine.revision(), error),
            }
        };
        // Представление строится только после зафиксированной команды. Его сбой
        // не откатывает мир и записывается отдельно в presentation_error.
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
        // Кэшируем и успехи, и отклонения: один command_id на протяжении этого
        // поколения сессии всегда обозначает один и тот же запрос и результат.
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

    /// Обрабатывает действие пользователя над текущим представлением.
    ///
    /// В отличие от [`Session::dispatch`], вход здесь описывает интерфейсное
    /// намерение: активировать элемент, выбрать клетку, навести курсор или
    /// закрыть окно. Lua-адаптер `interact` решает, достаточно ли локально
    /// обновить представление или нужно породить смысловую игровую команду.
    pub fn interact(&mut self, viewer: &str, interaction: Interaction) -> InteractionDelivery {
        if viewer.is_empty() {
            return interaction_error(
                &interaction.interaction_id,
                "invalid_input",
                "viewer id must not be empty",
            );
        }
        // UI-взаимодействия, как и команды, могут повторно прийти от транспорта.
        // Точный повтор безопасно получает прежний результат.
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
        // Взаимодействие привязано к той версии интерфейса, которую видел
        // пользователь. Нельзя применить к уже заменённой кнопке старый клик.
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
        // Сохраняем окончательный ответ независимо от успеха, чтобы повтор не
        // запускал интерпретацию и возможную игровую команду ещё раз.
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

    /// Выполняет ещё не встречавшееся взаимодействие без логики дедупликации.
    fn interact_once(
        &mut self,
        viewer: &str,
        interaction: &Interaction,
    ) -> Result<InteractionDelivery, Error> {
        // Контекст принадлежит конкретному viewer и никогда не смешивается с
        // контекстами других игроков или наблюдателей.
        let context = self
            .viewers
            .get(viewer)
            .and_then(|state| state.context.clone())
            .unwrap_or_else(|| Value::Map(BTreeMap::new()));
        // Протокольный enum преобразуется в устойчивые строковые значения Lua.
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
        // Value хранит целые как i64, а счётчик движка — как u64. Переполнение
        // лучше вернуть как конфликт, чем молча исказить номер мира.
        let world_revision = i64::try_from(self.engine.revision())
            .map_err(|_| Error::new("conflict", "world revision exceeds Value integer range"))?;
        let request = Value::Map(BTreeMap::from([
            ("viewer".into(), Value::String(viewer.into())),
            ("view_context".into(), context),
            ("world_revision".into(), Value::Integer(world_revision)),
            ("input".into(), input),
        ]));
        // `interact` является читающим скриптом: он может вычислить команду и
        // новый UI-контекст, но не может напрямую менять авторитетный мир.
        let result = parse_interaction_result(
            self.engine
                .read_script("interact", request)
                .map_err(engine_error)?,
        )
        .map_err(|message| Error::new("script_error", message))?;
        // Новый контекст сохраняется до построения следующего представления.
        self.viewers.entry(viewer.into()).or_default().context = Some(result.0);
        // Если интерпретатор породил Action, он проходит обычный dispatch со
        // всеми проверками и транзакцией. Синтетический ID связывает команду с
        // исходным взаимодействием и тоже делает повтор безопасным.
        let command = result.1.map(|action| {
            self.dispatch(
                viewer,
                Command {
                    command_id: format!("interaction:{}", interaction.interaction_id),
                    expected_world_revision: self.engine.revision(),
                    action: action.action,
                    payload: action.payload,
                },
            )
        });
        // Игровая команда сама построит ViewUpdate из своих событий. Если же мир
        // менять не понадобилось, обновляем только локальное представление.
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

    /// Строит и применяет инкрементальное обновление представления.
    ///
    /// В отличие от [`Session::snapshot`], возвращает только заменённые блоки,
    /// удалённые ID и одноразовые эффекты, а также указывает базовую ревизию,
    /// поверх которой клиент обязан применить патч.
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
                // Для Lua контракт всегда одинаков: даже одно событие
                // представляется списком.
                match events {
                    Value::List(events) => Value::List(events),
                    event => Value::List(vec![event]),
                },
            ),
            ("command_id".into(), Value::String(command_id.into())),
        ]));
        let presentation = parse_presentation(
            self.engine
                .read_script("present", request)
                .map_err(engine_error)?,
        )
        .map_err(|message| Error::new("script_error", message))?;
        let state = self.viewers.entry(viewer.into()).or_default();
        let base = state.revision;
        // Сначала применяем патч к копии. Настоящее состояние наблюдателя будет
        // изменено лишь после разбора и полной проверки результата.
        let mut blocks = state.blocks.clone();
        for id in &presentation.remove_ids {
            blocks.remove(id);
        }
        for block in &presentation.replace_blocks {
            blocks.insert(block.id.clone(), block.content.clone());
        }
        // Эффекты одноразовые и сами по себе считаются изменением, даже если
        // долговременные блоки остались прежними.
        let changed = blocks != state.blocks || !presentation.effects.is_empty();
        if !changed {
            return Ok(None);
        }
        let revision = base
            .checked_add(1)
            .ok_or_else(|| Error::new("conflict", "view revision exhausted"))?;
        let update = ViewUpdate {
            world_revision: self.engine.revision(),
            base_view_revision: base,
            view_revision: revision,
            replace_blocks: presentation.replace_blocks,
            remove_ids: presentation.remove_ids,
            effects: presentation.effects,
        };
        // До фиксации локальной ревизии проверяем все ссылки на изображения и
        // другие ресурсы. Ошибочный патч не портит сохранённое состояние viewer.
        protocol::validate_assets(&Message::ViewUpdate(update.clone()), &self.assets)?;
        state.blocks = blocks;
        state.revision = revision;
        Ok(Some(update))
    }

    /// Атомарно заменяет мир сохранённым снимком и начинает новое поколение.
    fn restore_store(&mut self, snapshot: StoreSnapshot) -> Result<(), Error> {
        // Engine сначала проверяет снимок и Lua-инварианты. При ошибке текущий
        // мир и вся протокольная история остаются нетронутыми.
        self.engine.restore(snapshot).map_err(engine_error)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| Error::new("conflict", "session generation exhausted"))?;
        self.generation = generation;
        // Новый ID не позволяет спутать сообщения до restore с сообщениями уже
        // восстановленной игры.
        self.id = format!("{}@{generation}", self.root_id);
        // Результаты старых команд, UI-контексты и номера представлений относятся
        // к прежнему миру и после восстановления недействительны.
        self.commands.clear();
        self.viewers.clear();
        self.interactions.clear();
        Ok(())
    }
}

/// Преобразует нетипизированный ответ Lua `present` в строгую структуру
/// протокола. Здесь намеренно отклоняются отсутствующие поля и значения неверных
/// типов: дальше код может работать с `Presentation` без дополнительных догадок.
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

/// Разбирает ответ Lua `interact`.
///
/// Первый элемент результата — новый локальный контекст наблюдателя. Второй —
/// необязательное смысловое действие, которое затем будет отправлено обычным
/// путём [`Session::dispatch`].
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

/// Общий строгий разбор списка Lua-record значений.
/// Используется для блоков представления и одноразовых эффектов.
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

/// Забирает обязательную непустую строку из разбираемой записи.
fn take_string(value: &mut BTreeMap<String, Value>, field: &str) -> Result<String, String> {
    match value.remove(field) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value),
        _ => Err(format!("presentation {field} must be a nonempty string")),
    }
}

/// Оставляет для игрового Lua только смысловую часть команды.
///
/// `command_id` и ожидаемая ревизия являются транспортными данными: сессия уже
/// обработала их и игровым правилам они не нужны.
fn command_value(command: &Command) -> Value {
    Value::Map(BTreeMap::from([
        ("action".into(), Value::String(command.action.clone())),
        ("payload".into(), command.payload.clone()),
    ]))
}

/// Создаёт стандартный ответ об отклонении команды из кода и текста ошибки.
fn rejected(command: &Command, revision: u64, code: &str, message: impl Into<String>) -> Dispatch {
    rejected_with_error(command, revision, Error::new(code, message))
}

/// Создаёт отклонённый `Dispatch`, когда готовая протокольная ошибка уже есть.
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

/// Создаёт ошибку взаимодействия из кода и сообщения.
fn interaction_error(
    interaction_id: &str,
    code: &str,
    message: impl Into<String>,
) -> InteractionDelivery {
    interaction_failure(interaction_id, Error::new(code, message))
}

/// Упаковывает готовую ошибку в ответ на UI-взаимодействие.
fn interaction_failure(interaction_id: &str, error: Error) -> InteractionDelivery {
    InteractionDelivery {
        interaction_id: interaction_id.into(),
        view: None,
        command: None,
        error: Some(error),
    }
}

/// Переводит внутреннюю ошибку движка в ошибку публичного протокола без потери
/// стабильного машинного кода.
fn engine_error(error: EngineError) -> Error {
    Error::new(error.code, error.message)
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
        let mut session = Session::load_entry(SessionInput {
            id: "test".into(),
            package: PackageIdentity {
                package_id: "test".into(),
                package_version: "1".into(),
                protocol_version: 1,
            },
            assets: BTreeSet::new(),
            world: World {
                map: Map {
                    width: 1,
                    height: 1,
                    cells: vec![Value::Map(BTreeMap::new())],
                },
                entities: BTreeMap::new(),
                data: BTreeMap::new(),
            },
            seed: 1,
            modules: BTreeMap::from([(
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
            entry: "game.init".into(),
        })
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
        assert_eq!(session.world().data["count"], Value::Integer(1));

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
        assert_eq!(session.world().data["count"], Value::Integer(2));

        let saved = session.save().unwrap();
        let original_id = session.id().to_owned();
        session.dispatch("viewer", command("c3", 2, Value::Null));
        assert_eq!(session.world().data["count"], Value::Integer(3));
        session.restore(&saved).unwrap();
        assert_ne!(session.id(), original_id);
        let restored_id = session.id().to_owned();
        assert_eq!(session.world().data["count"], Value::Integer(2));
        let mut invalid: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        invalid["store"]["world"]["data"]["count"] = 99.into();
        assert!(
            session
                .restore(&serde_json::to_vec(&invalid).unwrap())
                .is_err()
        );
        assert_eq!(session.id(), restored_id);
        assert_eq!(session.world().data["count"], Value::Integer(2));
    }

    #[test]
    fn presentation_failure_does_not_repeat_a_committed_command_and_snapshot_recovers() {
        let mut session = Session::load_entry(SessionInput {
            id: "presentation-failure".into(),
            package: PackageIdentity {
                package_id: "test".into(),
                package_version: "1".into(),
                protocol_version: 1,
            },
            assets: BTreeSet::new(),
            world: World {
                map: Map {
                    width: 1,
                    height: 1,
                    cells: vec![Value::Map(BTreeMap::new())],
                },
                entities: BTreeMap::new(),
                data: BTreeMap::new(),
            },
            seed: 1,
            modules: BTreeMap::from([(
                "game.init".into(),
                "return {
                    dispatch=function(c)
                        c.state:set('count',(c.state:get('count') or 0)+1)
                        return wesnoth.value.null
                    end,
                    present=function(c,r)
                        if r.command_id == 'c1' then error('presentation failed') end
                        return {
                            replace_blocks={{id='count',content=c.state:get('count')}},
                            remove_ids=wesnoth.value.list(), effects=wesnoth.value.list()
                        }
                    end,
                    interact=function(_,r) return {view_context=r.view_context} end,
                    validate_restored=function() return true end
                }"
                .into(),
            )]),
            entry: "game.init".into(),
        })
        .unwrap();
        let request = command("c1", 0, Value::Null);
        let failed = session.dispatch("viewer", request.clone());
        assert_eq!(failed.result.status, CommandStatus::Committed);
        assert_eq!(session.world().data["count"], Value::Integer(1));
        assert!(failed.view.is_none() && failed.presentation_error.is_some());
        assert_eq!(session.dispatch("viewer", request), failed);
        assert_eq!(session.world().data["count"], Value::Integer(1));

        let snapshot = session.snapshot("viewer").unwrap();
        assert_eq!(snapshot.world_revision, 1);
        assert_eq!(snapshot.blocks[0].content, Value::Integer(1));
    }
}
