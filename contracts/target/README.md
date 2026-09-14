# Реестр целевых контрактов

Статусы реализации указаны в отдельных контрактах; неотмеченные контракты остаются целевыми.
[Общие соглашения](common.md) · [Поток сообщений](stream.md) ·
[Архитектура](../../design/architecture/README.md).

Каждая карточка модуля ссылается на его контракт, контракт — на поставщика,
потребителей и используемые контракты. Семейства terrain используют один общий
контракт family.compile: одинаковый интерфейс не дублируется в девяти файлах.

| Модуль | Контракт | Карточка |
|---|---|---|
| client/app | [API и гарантии](modules/client/app.md) | [Ответственность](../../design/architecture/modules/client/app.md) |
| client/connection | [API и гарантии](modules/client/connection.md) | [Ответственность](../../design/architecture/modules/client/connection.md) |
| client/effects | [API и гарантии](modules/client/effects.md) | [Ответственность](../../design/architecture/modules/client/effects.md) |
| client/input | [API и гарантии](modules/client/input.md) | [Ответственность](../../design/architecture/modules/client/input.md) |
| client/scene | [API и гарантии](modules/client/scene.md) | [Ответственность](../../design/architecture/modules/client/scene.md) |
| client/ui | [API и гарантии](modules/client/ui.md) | [Ответственность](../../design/architecture/modules/client/ui.md) |
| client/view | [API и гарантии](modules/client/view.md) | [Ответственность](../../design/architecture/modules/client/view.md) |
| client/viewport | [API и гарантии](modules/client/viewport.md) | [Ответственность](../../design/architecture/modules/client/viewport.md) |
| engine/data_format | [API и гарантии](modules/engine/data_format.md) | [Ответственность](../../design/architecture/modules/engine/data_format.md) |
| engine/hex | [API и гарантии](modules/engine/hex.md) | [Ответственность](../../design/architecture/modules/engine/hex.md) |
| engine/pathfinding | [API и гарантии](modules/engine/pathfinding.md) | [Ответственность](../../design/architecture/modules/engine/pathfinding.md) |
| engine/persistence | [API и гарантии](modules/engine/persistence.md) | [Ответственность](../../design/architecture/modules/engine/persistence.md) |
| engine/protocol | [API и гарантии](modules/engine/protocol.md) | [Ответственность](../../design/architecture/modules/engine/protocol.md) |
| engine/resources | [API и гарантии](modules/engine/resources.md) | [Ответственность](../../design/architecture/modules/engine/resources.md) |
| engine/runtime | [API и гарантии](modules/engine/runtime.md) | [Ответственность](../../design/architecture/modules/engine/runtime.md) |
| engine/session | [API и гарантии](modules/engine/session.md) | [Ответственность](../../design/architecture/modules/engine/session.md) |
| engine/store | [API и гарантии](modules/engine/store.md) | [Ответственность](../../design/architecture/modules/engine/store.md) |
| engine/value | [API и гарантии](modules/engine/value.md) | [Ответственность](../../design/architecture/modules/engine/value.md) |
| lua/ai | [API и гарантии](modules/lua/ai.md) | [Ответственность](../../design/architecture/modules/lua/ai.md) |
| lua/bootstrap | [API и гарантии](modules/lua/bootstrap.md) | [Ответственность](../../design/architecture/modules/lua/bootstrap.md) |
| lua/campaign | [API и гарантии](modules/lua/campaign.md) | [Ответственность](../../design/architecture/modules/lua/campaign.md) |
| lua/combat | [API и гарантии](modules/lua/combat.md) | [Ответственность](../../design/architecture/modules/lua/combat.md) |
| lua/economy | [API и гарантии](modules/lua/economy.md) | [Ответственность](../../design/architecture/modules/lua/economy.md) |
| lua/effects | [API и гарантии](modules/lua/effects.md) | [Ответственность](../../design/architecture/modules/lua/effects.md) |
| lua/entry | [API и гарантии](modules/lua/entry.md) | [Ответственность](../../design/architecture/modules/lua/entry.md) |
| lua/flow | [API и гарантии](modules/lua/flow.md) | [Ответственность](../../design/architecture/modules/lua/flow.md) |
| lua/forecast | [API и гарантии](modules/lua/forecast.md) | [Ответственность](../../design/architecture/modules/lua/forecast.md) |
| lua/hud | [API и гарантии](modules/lua/hud.md) | [Ответственность](../../design/architecture/modules/lua/hud.md) |
| lua/interaction | [API и гарантии](modules/lua/interaction.md) | [Ответственность](../../design/architecture/modules/lua/interaction.md) |
| lua/map | [API и гарантии](modules/lua/map.md) | [Ответственность](../../design/architecture/modules/lua/map.md) |
| lua/movement | [API и гарантии](modules/lua/movement.md) | [Ответственность](../../design/architecture/modules/lua/movement.md) |
| lua/presenter | [API и гарантии](modules/lua/presenter.md) | [Ответственность](../../design/architecture/modules/lua/presenter.md) |
| lua/scenario | [API и гарантии](modules/lua/scenario.md) | [Ответственность](../../design/architecture/modules/lua/scenario.md) |
| lua/sides | [API и гарантии](modules/lua/sides.md) | [Ответственность](../../design/architecture/modules/lua/sides.md) |
| lua/terrain_rendering | [API и гарантии](modules/lua/terrain_rendering.md) | [Ответственность](../../design/architecture/modules/lua/terrain_rendering.md) |
| lua/terrain | [API и гарантии](modules/lua/terrain.md) | [Ответственность](../../design/architecture/modules/lua/terrain.md) |
| lua/time | [API и гарантии](modules/lua/time.md) | [Ответственность](../../design/architecture/modules/lua/time.md) |
| lua/turns | [API и гарантии](modules/lua/turns.md) | [Ответственность](../../design/architecture/modules/lua/turns.md) |
| lua/units | [API и гарантии](modules/lua/units.md) | [Ответственность](../../design/architecture/modules/lua/units.md) |
| lua/visibility | [API и гарантии](modules/lua/visibility.md) | [Ответственность](../../design/architecture/modules/lua/visibility.md) |

## Прежние документы

Эти документы сохраняют детали прежнего API и семантику текущей игры.
Они не задают границы новой архитектуры. При переносе уточняются нужные типы
целевого контракта и закрывается соответствующий старый API.

| Прежний документ | Целевые владельцы |
|---|---|
| [world](../world.md) | engine/store, hex, value; игровая terrain-семантика → lua/terrain |
| [lua-runtime](../lua-runtime.md) | engine/runtime, store; lua/entry |
| [platform](../platform.md) | stream, engine/protocol/session, client/connection, Lua presentation |
| [scenario-data](../scenario-data.md) | lua/bootstrap, scenario, units, sides, terrain |
| [campaign](../campaign.md) | lua/campaign, scenario; engine/persistence |
| [adventures](../adventures.md) | lua/campaign, hud; engine/resources |
| [terrain-rendering](../terrain-rendering.md) | lua/terrain_rendering, map; client/scene |
| [first-battle](../first-battle.md) | Сквозной критерий lua/flow/combat/scenario, не отдельный модуль |

Формат WML: [справочник](../../docs/wml-reference.md).
Семантика боя: [описание](../../docs/combat-architecture.md) и
[проверки соответствия](../../docs/combat-parity.md). Они помогают сохранить
поведение, но не разрешают оставлять игровые формулы в Rust после M04.
