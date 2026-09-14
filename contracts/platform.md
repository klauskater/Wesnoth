# Контракт платформы

Статус: переходная документация прежнего API и семантики игры;
полный аудит соответствия коду ещё не завершён. Целевые межмодульные границы
и владельцы описаны в [реестре контрактов](target/README.md#прежние-документы).

Платформа показывает состояние и отправляет намерения пользователя. Она не
решает, можно ли двигаться, атаковать, нанимать или завершать ход: эти решения
принадлежат Lua-правилам.

## Rust-граница

```text
Game::map() -> &Map
Game::map_tiles() -> &MapTiles
Game::terrain_scene() -> &TerrainScene
Game::snapshot() -> { map, objects }
Game::query(function, command) -> Value
Game::execute(function, command) -> [event]
Game::start_events() -> [event]
Game::acknowledge_dialog()
Game::save() -> JSON
```

`query` не изменяет игру. `execute` фиксирует одну транзакцию и всегда
нормализует результат к списку событий. Ошибка возвращается строкой и ничего не
фиксирует.

Если успешный список содержит событие с полем `dialog`, `Game` ставит этот
диалог в ожидание и добавляет `dialog_requested`. Пока он не подтверждён,
следующий `execute` отклоняется. `query`, `snapshot` и `save` остаются доступны.

## Запросы Lua

### `status`

Вход: `nil`.

```text
{
  turn, active_side, finished, can_end_turn,
  phase, objective, result, turn_limit, carryover_percentage,
  gold,
  recruit_types, recruit_options, recruit_hexes,
  recall_units, recall_options,
  villages,
  income, base_income, villages_owned, village_support,
  upkeep, support, expenses, gross_income, net_income,
  pending_advancement, pending_choice,
  time_of_day, lawful_bonus,
  fog, visible_cells, visible_units, shroud, revealed_cells
}
```

`recruit_hexes` содержит `{x,y,leader}`. Варианты найма и recall содержат
идентификаторы, цену, HP/MP, уровень, мировоззрение, опыт где применимо и
описания атак. Отсутствующие необязательные значения приходят как `nil`.
`pending_advancement` имеет форму `{unit, options, details}`: `options` хранит
допустимые id типов, а `details` — их отображаемые характеристики для окна UI.

### `snapshot`

Вход: `nil`. Результат — список видимых для движка объектов независимо от fog;
фильтрацию отображения делает клиент по `status.visible_units`.

Каждый объект содержит:

```text
{
  id, type, name, side, is_leader, position,
  hitpoints, max_hitpoints,
  movement_points, max_movement_points, attacks_left,
  level, experience, max_experience, alignment,
  movement_costs, defense, resistances,
  poisoned, slowed, petrified, unhealable, stunned,
  attacks: [{ id, name, damage, strikes, range, damage_type, specials }]
}
```

### `reachable`

Вход: `{ object, inspect? }`.

Результат: `[{ position, cost, path, zoc, teleport? }]`. При `inspect=true`
разрешено смотреть чужой объект и используется полный `max_moves`; иначе
проверяются активная сторона и текущие MP. Занятая клетка не является конечной.
Союзная клетка может участвовать в поиске пути. Непроходимая местность не имеет
стоимости в `[movement_costs]`.

Если стоимость клетки превышает оставшиеся MP, но не полный `max_moves`, вход
забирает весь остаток. Зона контроля также обнуляет остаток. Телепорт между
свободными союзными деревнями стоит 1 MP.

### `actions`

Вход: `{ object, inspect? }`.

```text
{
  reachable: результат reachable,
  targets: [видимые соседние enemy id],
  attacks: [weapon id]
}
```

При `inspect=true` цели пусты, но возвращаются характеристики движения и атак.

### `preview_attack`

Вход: `{ attacker, defender, weapon, position? }`. `position` позволяет
оценить атаку после предполагаемого перемещения, не изменяя объект.

```text
{
  chance, damage, strikes, expected_damage, kill_probability,
  retaliation_chance, retaliation_damage, retaliation_strikes,
  expected_retaliation, death_probability, retaliation_name,
  time_of_day, alignment_modifier, next_alignment_modifier
}
```

## Команды Lua

| Функция | Команда | Основные проверки |
| --- | --- | --- |
| `move` | `{ object, destination:{x,y} }` | destination присутствует в `reachable` |
| `resolve` | `{ attacker, defender, weapon }` | активная сторона, живые соседние враги, видимость, AP и состояния |
| `end_turn` | `nil` | активная сторона управляется человеком |
| `recruit` | `{ unit_type, destination?, leader? }` | список найма, лидер на keep, связный свободный castle, золото |
| `recall` | `{ unit, destination, leader? }` | человек, боец в recall, крепость и золото |
| `advance` | `{ unit, choice }` | ожидается выбор повышения, сценарий ещё не завершён и тип разрешён |
| `choose` | `{ choice, option }` | ожидается соответствующий сценарный выбор |

`move`, `resolve`, `end_turn`, `recruit` и `recall` запрещены во время
сценарного выбора или выбора повышения согласно проверкам конкретной функции.
После `scenario_finished` изменяющие команды запрещены. Если победный бой требует
выбора повышения, `scenario_finished` откладывается до обработки последнего
`advance`.

## События

События возвращаются в порядке уже совершившихся изменений. Клиент может их
анимировать, но не должен повторно применять к миру.

### Диалог и сценарий

```text
dialog_requested { dialog, lines:[{speaker,text}] }
phase_changed { phase, dialog?, spawned:[id] }
turn_event { turn, id?, dialog?, spawned:[id] }
choice_required { choice, options:[id], dialog?, turn?, spawned? }
choice_resolved { choice, option, correct, removed:[id], dialog? }
location_event { id, object, dialog?, gold?, amount?, damage?, hitpoints?, status?, achievement? }
attack_event { id, attacker, defender, dialog?, achievement? }
achievement_unlocked { id }
scenario_finished { result, dialog?, next_scenario?, achievements:[id] }
```

### Ход и экономика

```text
turn_ended { turn, side }
turn_started { turn, side }
economy_updated {
  side, gold, base_income, villages_owned, village_income,
  village_support, gross_income, upkeep, support, expenses, net_income
}
unit_healed { unit, amount, hitpoints, source }
poison_damage { unit, damage, hitpoints }
status_cured { unit, status }
status_expired { unit, status }
```

### Перемещение и состав армии

```text
object_moved {
  object, from, to, path, cost, movement_points,
  stopped_by_zoc, teleported
}
village_captured { object, side, position }
unit_recruited { unit, unit_type, position, cost, gold }
unit_recalled { unit, position, cost, gold }
advancement_required { unit, from, options:[type] }
unit_advanced { unit, from, to, level, experience, maximum, hitpoints }
unit_died { unit, side, position }
```

Захват деревни всегда оставляет `object_moved.movement_points=0`.

### Бой

```text
battle_resolved {
  attacker, defender,
  strikes:[strike], defeated?,
  experience:[{unit,gained,total,maximum}],
  berserk
}
```

Каждый `strike` содержит:

```text
{
  number, source, target, weapon,
  chance, roll, hit,
  damage, base_damage, modified_damage, damage_type,
  resistance, effective_resistance,
  alignment, alignment_modifier, leadership_bonus, time_of_day,
  slowed_damage,
  poisoned, slowed, petrified, stunned, drained,
  target_hitpoints, source_experience
}
```

Урон, статусы и смерть уже применены к моменту получения события.

## Обязанности клиента

Клиент:

- строит интерфейс из `status`, `snapshot`, `actions` и `preview_attack`;
- не показывает объект врага вне `visible_units` и затемняет клетки по fog/shroud;
- блокирует новый ввод на время локальной анимации и ожидающего диалога;
- после каждой команды перечитывает состояние, не реконструируя его из событий;
- передаёт кампанийное состояние следующей главе и умеет сохранить/загрузить
  текущую партию.

Конкретные мышь, клавиши, Android gestures, размеры и текстуры не входят в
контракт движка.

Статическая карта и описания её тайлов читаются один раз при открытии сценария.
Платформенный `MapRenderer` строит из них локальный кеш и рисует его без вызовов
Lua и без повторного получения `snapshot` в кадровом цикле.
