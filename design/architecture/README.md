# Целевая модульная архитектура

Статус: проект структуры от 2026-09-13, подготовленный для обсуждения и последовательного переноса.
Базовые Rust-модули engine переносятся по этому решению; актуальный статус указан
в карточках модулей. Текущие публичные API описаны в [contracts](../../contracts/README.md),
а этот каталог описывает целевое устройство.
При расхождении нельзя считать проект уже реализованным.

## Основное решение

Rust — универсальный хост, авторитетное хранилище и вычислительные примитивы.
Lua-пакет — вся семантика игры, включая загрузку игровых сущностей, правила,
сценарии, AI и описание представления. Клиент — ввод и воспроизведение готового
представления общими визуальными компонентами.

Модуль — часть с одной ответственностью и явным API, а не отдельный процесс,
crate, сервис или плагин. Сохраняем один Rust crate и один пакет текущей игры.
Ниже каждому будущему модулю соответствует отдельная карточка и конкретный путь.
В реализации ответственность обязательно находится также в заголовке самого файла.
Каждый модуль связан с [его контрактом](../../contracts/target/README.md):
API и гарантии задаются там, а в карточке остаются архитектурные границы.
Пустые .rs/.lua-заглушки заранее не создаются: они смешали бы проект и рабочий код.

## Структура модулей

### engine

| Файл реализации | Ответственность |
|---|---|
| [`src/engine/protocol.rs`](modules/engine/protocol.md) | Общий словарь границы движок–клиент. |
| [`src/engine/store.rs`](modules/engine/store.md) | Единственное авторитетное хранилище изменяемых данных игры. |
| [`src/engine/runtime.rs`](modules/engine/runtime.md) | Исполнение модулей Lua с ограниченными возможностями доступа. |
| [`src/engine/session.rs`](modules/engine/session.md) | Оркестрация жизненного цикла сессии и последовательной обработки входа. |
| [`src/engine/resources.rs`](modules/engine/resources.md) | Доступ к содержимому сменяемого пакета игры. |
| [`src/engine/data_format.rs`](modules/engine/data_format.md) | Синтаксический разбор декларативных ресурсов. |
| [`src/engine/persistence.rs`](modules/engine/persistence.md) | Запись и восстановление согласованного снимка сессии. |
| [`src/engine/hex.rs`](modules/engine/hex.md) | Геометрия гексагональной сетки. |
| [`src/engine/pathfinding.rs`](modules/engine/pathfinding.md) | Пакетный поиск по заданному графу гексов. |
| [`src/engine/value.rs`](modules/engine/value.md) | Общие сериализуемые значения и преобразование на границе Lua. |

### client

| Файл реализации | Ответственность |
|---|---|
| [`src/bin/client/app.rs`](modules/client/app.md) | Запуск платформенного клиента и соединение его частей. |
| [`src/bin/client/connection.rs`](modules/client/connection.md) | Адаптация вызовов встроенной сессии к протоколу клиента. |
| [`src/bin/client/view.rs`](modules/client/view.md) | Локальная копия готового представления для отрисовки. |
| [`src/bin/client/input.rs`](modules/client/input.md) | Преобразование физического ввода в универсальные взаимодействия. |
| [`src/bin/client/ui.rs`](modules/client/ui.md) | Раскладка и отрисовка декларативного интерфейса. |
| [`src/bin/client/scene.rs`](modules/client/scene.md) | Рисование готовой сцены и загрузка её графических ресурсов. |
| [`src/bin/client/viewport.rs`](modules/client/viewport.md) | Камера и преобразования экранных координат. |
| [`src/bin/client/effects.rs`](modules/client/effects.md) | Воспроизведение одноразовых визуальных и звуковых эффектов. |

### lua

| Файл реализации | Ответственность |
|---|---|
| [`scripts/game/init.lua`](modules/lua/entry.md) | Единственная внешняя точка входа пакета и явная сборка модулей. |
| [`scripts/game/bootstrap.lua`](modules/lua/bootstrap.md) | Преобразование декларативных данных именно этой игры в начальный мир. |
| [`scripts/game/flow.lua`](modules/lua/flow.md) | Порядок выполнения одной игровой команды и связанных последствий. |
| [`scripts/game/rules/units.lua`](modules/lua/units.md) | Создание и изменение юнитов по каталогам, traits и развитию. |
| [`scripts/game/rules/sides.lua`](modules/lua/sides.md) | Стороны, принадлежность и отношения между ними. |
| [`scripts/game/rules/terrain.lua`](modules/lua/terrain.md) | Игровая семантика местности этой игры. |
| [`scripts/game/rules/movement.lua`](modules/lua/movement.md) | Правила перемещения и достижимости. |
| [`scripts/game/rules/combat.lua`](modules/lua/combat.md) | Разрешение боя и общие для выполнения/прогноза боевые расчёты. |
| [`scripts/game/rules/forecast.lua`](modules/lua/forecast.md) | Прогноз вероятностей исходов боя без изменения мира. |
| [`scripts/game/rules/time.lua`](modules/lua/time.md) | Игровой календарь и влияние времени суток. |
| [`scripts/game/rules/turns.lua`](modules/lua/turns.md) | Очередность ходов и процедуры их начала/окончания. |
| [`scripts/game/rules/economy.lua`](modules/lua/economy.md) | Золото, доход, содержание, владение деревнями и найм/возврат. |
| [`scripts/game/rules/visibility.lua`](modules/lua/visibility.md) | Обзор, туман, исследованность и скрытые объекты. |
| [`scripts/game/scenario.lua`](modules/lua/scenario.md) | Сценарные триггеры, цели, диалоги и обязательные выборы. |
| [`scripts/game/campaign.lua`](modules/lua/campaign.md) | Каталог приключений и правила перехода между сценариями. |
| [`scripts/game/ai.lua`](modules/lua/ai.md) | Выбор допустимых команд для компьютерной стороны. |
| [`scripts/game/presentation/presenter.lua`](modules/lua/presenter.md) | Сборка представления конкретного наблюдателя после изменения мира или взаимодействия. |
| [`scripts/game/presentation/interaction.lua`](modules/lua/interaction.md) | Игровое значение кликов и контекст просмотра. |
| [`scripts/game/presentation/map.lua`](modules/lua/map.md) | Подготовка всей видимой сцены гекс-поля. |
| [`scripts/game/presentation/terrain/init.lua`](modules/lua/terrain_rendering.md) | Сборка визуальных семейств местности и их порядка. |
| [`scripts/game/presentation/hud.lua`](modules/lua/hud.md) | Описание игрового интерфейса и игровых меню через общие UiNode. |
| [`scripts/game/presentation/effects.lua`](modules/lua/effects.md) | Перевод игровых событий в видимые одноразовые эффекты. |

## Направление зависимостей

```text
client/app -> client/* -> engine/protocol
client/connection -> engine/session
engine/session -> runtime, store, resources, persistence
engine/runtime -> store, resources, hex, pathfinding, value
Lua init -> bootstrap, flow, presentation/presenter, presentation/interaction
Lua flow -> rules/*, scenario, campaign, ai
Lua presenter -> map, hud, effects
Lua presentation/* и ai -> rules/*
Lua rules/* -> разрешённые нижние rules и host API
```

Карточки уточняют разрешённые зависимости. Ни один rule не импортирует flow,
presentation или ai. combat не импортирует forecast; forecast использует чистые
вычисления combat. scenario возвращает последствия в flow, а не импортирует его.
turns не вызывает AI: flow организует продолжение. runtime предоставляет
возможности host через ctx, без импортов Rust-модулей из Lua.

Внешняя точка входа одна — game/init.lua, с четырьмя операциями:
initialize, dispatch, interact и present. Interact интерпретирует ввод,
present только строит отображение и не создаёт команды. Остальные Lua-вызовы между модулями
происходят внутри VM, без возвращения управления в Rust на каждую функцию.
Контролируемый require нужен при переносе: текущая VM его отключает, а загрузчик
склеивает исходники. Обычный require с доступом к файловой системе не включается.

## Данные пакета

Сохраняем существующие каталоги scripts/adventures, scenarios, dialogs, maps,
map_objects, units и assets; перенос данных ради красоты не нужен.
Добавляется manifest пакета с id, версией, entry и ссылками на ресурсы.
Rust разбирает синтаксис и предоставляет ресурсы, Lua определяет значение тегов.
Каталоги нельзя жёстко перечислять в Engine как unit_types/races/traits:
bootstrap регистрирует произвольные неизменяемые ресурсы.

Существующие scripts/terrain/*.lua при переносе становятся соседями
presentation/terrain/init.lua. Список и ответственность каждого семейства:
grass — базовый травяной покров; hills — холмы и их переходы; water — вода и берег;
forest — лесной покров; road — дороги и соединения; dirt — грунт и переходы;
decorations — декоративные спрайты; bridges — мосты и ориентация;
castles — стены, башни и соединения замков.
Каждый файл сохраняет собственный модульный комментарий и возвращает спрайты.
Их визуальные правила не смешиваются с rules/terrain.lua.

## Где находится состояние

| Данные | Владелец | Сохранение |
|---|---|---|
| Мир, сценарий, кампания, обязательные игровые выборы, RNG | engine/store; схему задаёт Lua | Да |
| Каталоги и изображения | resources | Ссылка на id/версию пакета |
| Выбор/hover, открытая информационная панель наблюдателя | session view context; схему задаёт interaction.lua | Не часть мира |
| Камера, масштаб, прокрутка, фокус | Клиент | Только локальные предпочтения при необходимости |
| Дерево UI, сцена, кеш прогнозов | Восстанавливаемые кеши | Нет |

Выбор развития бойца или сценарный выбор — состояние игры, даже если выглядит
как окно. Просмотр вкладки или выбор бойца для инспекции — контекст представления.
Только store определяет игровые факты; изменяемые globals/upvalues Lua не могут
быть вторым хранилищем. Кеш допустим, если его удаление не меняет результат игры.

## Сопутствующие решения

- [Протокол и цикл обработки](protocol.md).
- [План переноса и критерии готовности](migration.md).

Не вводим ECS, глобальный event bus, сетевой стек, универсальный reactive graph,
отдельный модуль для каждой способности или виджета. Малые связанные функции
остаются в ответственном модуле. Новые файлы появляются при появлении отдельной
ответственности; число строк само по себе не является границей.
