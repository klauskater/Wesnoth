//! Публичная граница жизненного цикла игрового движка.
//!
//! Движок принимает смысловые игровые события (`Command` и `Interaction`) и
//! возвращает сообщения или снимки, описывающие изменения игрового мира. Движок
//! не знает, в какую координату экрана нажал игрок, как рисуется кнопка и как
//! проигрывается анимация. Преобразование физического ввода в игровые события и
//! отображение результата движка — обязанности интерфейса клиента.
//!
//! Запуск игры намеренно разделён на два этапа:
//!
//! 1. [`load_resources`] или [`load_resources_from`] разбирает выбранный сценарий
//!    и создаёт ещё не запущенный пакет [`EngineResources`].
//! 2. [`Game::start`] принимает этот пакет и создаёт новую либо восстанавливает
//!    сохранённую сессию движка.
//!
//! Благодаря этому менеджер ресурсов клиента может подготовить все данные до
//! запуска движка и интерфейса. Методы `Game::load*` ниже оставлены для тестов и
//! старого кода. Новый клиент должен запускать игру через отдельный загрузчик
//! ресурсов.

use std::{collections::BTreeMap, path::Path};

use crate::{
    adventure::{Adventure, AdventureScenario},
    engine::{
        Map,
        protocol::{Command, Interaction},
        session::{Dispatch, InteractionDelivery, Session, SessionInput},
    },
    map::MapTiles,
    terrain_scene::TerrainScript,
};

mod campaign;
mod compat;
mod load;
mod persistence;

pub use campaign::CampaignState;
pub use compat::{DialogLine, GameSnapshot};

/// Полностью подготовленный, но ещё не запущенный контекст движка для сценария.
///
/// Снаружи этого модуля содержимое намеренно скрыто. Загрузчик может разобрать
/// файлы и собрать правила, карту и начальное состояние, но только
/// [`Game::start`] превращает эти данные в работающую [`Session`]. Поэтому окно
/// или интерфейс не могут случайно собрать движок лишь наполовину.
pub struct EngineResources {
    id: String,
    name: String,
    start_dialog: String,
    session: SessionInput,
    initialization: Initialization,
    map_tiles: MapTiles,
    terrain_scripts: Vec<TerrainScript>,
    scene_assets: BTreeMap<String, String>,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
}

/// Определяет, как нужно запустить подготовленную сессию.
///
/// Оба варианта используют одинаково загруженные правила и сценарий. Разница
/// только в том, получит ли сессия начальный запрос новой игры или восстановит
/// ранее сохранённое состояние движка.
enum Initialization {
    New(crate::value::Value),
    Restore(Vec<u8>),
}

/// Загружает часть сценария, необходимую движку, из каталога скриптов.
///
/// Игровой цикл здесь не запускается. Возвращённый пакет можно объединить с
/// отдельно подготовленным контекстом интерфейса, а затем передать в
/// [`Game::start`].
pub fn load_resources(
    scripts: impl AsRef<Path>,
    scenario_path: &str,
    campaign: Option<&CampaignState>,
) -> Result<EngineResources, String> {
    let read = load::filesystem(scripts)?;
    load_resources_from(scenario_path, campaign, &read)
}

/// Вариант [`load_resources`], не привязанный к файловой системе.
///
/// Компьютерный клиент, мобильная сборка, тесты или другое хранилище передают
/// способ чтения через `read`. Сам порядок инициализации движка не меняется.
pub fn load_resources_from(
    scenario_path: &str,
    campaign: Option<&CampaignState>,
    read: &impl Fn(&str) -> Result<String, String>,
) -> Result<EngineResources, String> {
    load::scenario(scenario_path, campaign, read, None)
}

/// Загружает главу через корневой манифест приключения.
///
/// Именно этот путь должен использовать клиент: приключение выбирает правила и
/// доступные типы юнитов, глава выбирает процесс, карту и диалоги, а карта —
/// определения своих тайлов.
pub fn load_adventure_resources(
    scripts: impl AsRef<Path>,
    adventure: &Adventure,
    chapter: &AdventureScenario,
    campaign: Option<&CampaignState>,
) -> Result<EngineResources, String> {
    let read = load::filesystem(scripts)?;
    load_adventure_resources_from(adventure, chapter, campaign, &read)
}

/// Вариант [`load_adventure_resources`] для встроенного или иного хранилища.
pub fn load_adventure_resources_from(
    adventure: &Adventure,
    chapter: &AdventureScenario,
    campaign: Option<&CampaignState>,
    read: &impl Fn(&str) -> Result<String, String>,
) -> Result<EngineResources, String> {
    load::adventure(adventure, chapter, campaign, read, None)
}

/// Запущенный экземпляр игрового движка.
///
/// `Game` через свою сессию владеет единственным истинным изменяемым состоянием
/// мира. Внешний код передаёт смысловые игровые действия через
/// [`Game::dispatch_command`] или [`Game::interact`] и при необходимости
/// запрашивает полный снимок для конкретного наблюдателя. Состояние отрисовки
/// здесь храниться не должно.
pub struct Game {
    pub id: String,
    pub name: String,
    pub start_dialog: String,
    session: Session,
    map_tiles: MapTiles,
    terrain_scripts: Vec<TerrainScript>,
    scene_assets: BTreeMap<String, String>,
    dialogs: BTreeMap<String, Vec<DialogLine>>,
    compat: compat::State,
}

impl Game {
    /// Запускает ранее подготовленный контекст движка.
    ///
    /// Это единственный переход от загруженных ресурсов к работающей сессии.
    /// Метод либо открывает сценарий с его начальным запросом, либо восстанавливает
    /// сохранённое состояние, после чего оставляет у движка все невизуальные
    /// данные, необходимые для описания мира клиенту.
    pub fn start(resources: EngineResources) -> Result<Self, String> {
        let EngineResources {
            id,
            name,
            start_dialog,
            session,
            initialization,
            map_tiles,
            terrain_scripts,
            scene_assets,
            dialogs,
        } = resources;
        let session = match initialization {
            Initialization::New(request) => Session::open(session, request)?,
            Initialization::Restore(bytes) => Session::restore_entry(session, &bytes)?,
        };
        Ok(Self {
            id,
            name,
            start_dialog,
            session,
            map_tiles,
            terrain_scripts,
            scene_assets,
            dialogs,
            compat: compat::State::default(),
        })
    }

    // Временный слой совместимости загрузки ----------------------------------------
    //
    // Эти методы сохраняют старый одношаговый API для тестов, сохранений и ещё не
    // перенесённого кода. Внутри они всё равно используют единый путь
    // `EngineResources -> Game::start`. Графический клиент через них игру не
    // запускает.

    pub fn load(scripts: impl AsRef<Path>, scenario_path: &str) -> Result<Self, String> {
        Self::load_with_campaign(scripts, scenario_path, None)
    }

    pub fn load_with_campaign(
        scripts: impl AsRef<Path>,
        scenario_path: &str,
        campaign: Option<&CampaignState>,
    ) -> Result<Self, String> {
        let read = load::filesystem(scripts)?;
        Self::load_with_campaign_from(scenario_path, campaign, &read)
    }

    pub fn load_from(
        scenario_path: &str,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::load_with_campaign_from(scenario_path, None, read)
    }

    pub fn load_with_campaign_from(
        scenario_path: &str,
        campaign: Option<&CampaignState>,
        read: &impl Fn(&str) -> Result<String, String>,
    ) -> Result<Self, String> {
        Self::build_from(scenario_path, campaign, read, None)
    }

    fn build_from(
        scenario_path: &str,
        campaign: Option<&CampaignState>,
        read: &impl Fn(&str) -> Result<String, String>,
        saved: Option<&[u8]>,
    ) -> Result<Self, String> {
        Self::start(load::scenario(scenario_path, campaign, read, saved)?)
    }

    /// Создаёт полное описание текущего мира для конкретного наблюдателя.
    ///
    /// Это данные на выходе движка, а не готовая картинка интерфейса. Клиент сам
    /// решает, как превратить блоки протокола в элементы интерфейса, спрайты,
    /// слои и анимации.
    pub fn view_snapshot(
        &mut self,
        viewer: &str,
    ) -> Result<crate::engine::protocol::ViewSnapshot, String> {
        self.session.snapshot(viewer)
    }

    /// Выполняет смысловую игровую команду и возвращает вызванные ею сообщения.
    /// Координаты экрана и сырые события мыши или клавиатуры интерфейс обязан
    /// преобразовать до вызова этого метода.
    pub fn dispatch_command(&mut self, viewer: &str, command: Command) -> Dispatch {
        self.session.dispatch(viewer, command)
    }

    /// Продолжает взаимодействие, которое запросил движок, например выбор варианта.
    /// Движок проверяет и обрабатывает выбор. Интерфейс только показывает варианты
    /// и сообщает выбранное смысловое значение.
    pub fn interact(&mut self, viewer: &str, interaction: Interaction) -> InteractionDelivery {
        self.session.interact(viewer, interaction)
    }

    // Методы только для чтения описания мира. Они отдают логические данные карты
    // и сцены, необходимые для контекста клиента, но не объекты отрисовки.
    pub fn map(&self) -> &Map {
        &self.session.world().map
    }

    pub fn map_tiles(&self) -> &MapTiles {
        &self.map_tiles
    }

    pub fn terrain_scripts(&self) -> &[TerrainScript] {
        &self.terrain_scripts
    }

    pub fn scene_assets(&self) -> &BTreeMap<String, String> {
        &self.scene_assets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use std::path::PathBuf;

    fn scripts() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts")
    }

    #[test]
    fn successful_commands_advance_the_ui_revision() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        game.acknowledge_dialog().unwrap();
        let before = game.revision();
        let before_view = game.view_snapshot("time-test").unwrap();
        let before_time = before_view
            .blocks
            .iter()
            .find(|block| block.id == "time")
            .and_then(|block| crate::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[1].asset.clone())
            .unwrap();
        game.execute("end_turn", Value::Nil).unwrap();
        assert!(game.revision() > before);
        let after_view = game.view_snapshot("time-test").unwrap();
        let after_time = after_view
            .blocks
            .iter()
            .find(|block| block.id == "time")
            .and_then(|block| crate::engine::protocol::ui_block(&block.content).ok())
            .and_then(|root| root.children[1].asset.clone())
            .unwrap();
        assert_ne!(after_time, before_time);
    }

    #[test]
    fn package_builds_generic_view_blocks() {
        let mut game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let view = game.view_snapshot("test").unwrap();
        assert_eq!(
            view.blocks
                .iter()
                .map(|block| block.id.as_str())
                .collect::<Vec<_>>(),
            [
                "assets",
                "dialog",
                "hud",
                "map",
                "navigation",
                "objects",
                "scene",
                "status",
                "summary",
                "time"
            ]
        );
        let hud = view.blocks.iter().find(|block| block.id == "hud").unwrap();
        let root = crate::engine::protocol::ui_block(&hud.content).unwrap();
        assert_eq!(root.children[0].action.as_ref().unwrap().action, "end_turn");
        let time = view.blocks.iter().find(|block| block.id == "time").unwrap();
        let root = crate::engine::protocol::ui_block(&time.content).unwrap();
        assert_eq!(
            root.children[1].kind,
            crate::engine::protocol::UiKind::Image
        );
        assert!(
            root.children[1]
                .asset
                .as_deref()
                .unwrap()
                .starts_with("ui/time/")
        );
        assert!(matches!(time.content.get("map_tint"), Some(Value::List(tint)) if tint.len() == 4));
        let scene = view
            .blocks
            .iter()
            .find(|block| block.id == "scene")
            .unwrap();
        let items = crate::engine::protocol::scene_block(&scene.content).unwrap();
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| {
            matches!(item.layer.as_str(), "ground" | "world")
                && item.frames.first() == Some(&item.asset)
        }));
    }

    #[test]
    fn filesystem_loader_rejects_paths_outside_scripts() {
        let error = Game::load(scripts(), "../Cargo.toml").err().unwrap();
        assert!(error.contains("invalid package path"));
    }

    #[test]
    fn load_save_rejects_invalid_restored_state() {
        let game = Game::load(scripts(), "scenarios/first_battle.wml").unwrap();
        let mut save: serde_json::Value = serde_json::from_str(&game.save().unwrap()).unwrap();
        save["store"]["world"]["data"]
            .as_object_mut()
            .unwrap()
            .remove("scenario");
        let source = serde_json::to_string(&save).unwrap();
        let error = Game::load_save(scripts(), &source).err().unwrap();
        assert!(error.contains("invalid save state"));
    }
}
