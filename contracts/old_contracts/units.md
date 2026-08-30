# ⚠ Контракты юнитов

> [!WARNING]
> Устаревший справочный контракт официального Wesnoth. Не реализовывать как API нового движка без отдельного решения.

Юниты принадлежат слою правил и хранятся отдельно от универсальной карты.
Связь с картой — необязательная координата и занятость ячейки.

Опасность: перенос этого API в ядро карты сделает движок зависимым от модели
Wesnoth. Движок может хранить непрозрачный объект по координате, но создание,
бой, развитие, способности и фильтры должны оставаться в правилах/Lua.

## Коллекция `wesnoth.units`

- создание: `create(config)`, `clone(unit)`;
- поиск: `get(id)` или `get(x, y)`, `find_on_map(filter)`,
  `find_on_recall(filter)`, `find(filter)`, `matches(unit, filter)`;
- размещение: `to_map`, `to_recall`, `extract`, `erase`;
- жизненный цикл: `advance`, `transform`, `teleport`, `select`;
- изменения: `add_modification`, `remove_modifications`, `rebuild`;
- расчёты: `ability`, `defense_on`, `movement_on`, `vision_on`, `jamming_on`,
  `resistance_against`, `chance_to_be_hit`;
- фабрики: `create_weapon(config)`, `create_animator()`;
- только для UI: `get_hovered`.

## Объект юнита

| Доступ | Поля |
| --- | --- |
| `RW` | `x`, `y`, `loc`, `goto`, `side`, `id`, `usage`, `ellipse`, `halo`, `hitpoints`, `max_hitpoints`, `experience`, `max_experience`, `recall_cost`, `moves`, `max_moves`, `max_attacks`, `attacks_left`, `name`, `description`, `canrecruit`, `renamable`, `level`, `extra_recruit`, `advances_to`, `alignment`, `upkeep`, `advancements`, `recall_filter`, `hidden`, `resting`, `zoc`, `role`, `undead_variation`, `facing`, `portrait` |
| `R` | `valid`, `type`, `image_mods`, `vision`, `jamming`, `cost`, `overlays`, `traits`, `abilities`, `ability_ids`, `petrified`, `animations`, `flying`, `fearless`, `healthy`, `race`, `gender`, `variation`, `status`, `variables`, `attacks`, `__cfg` |

Смена позиции должна согласованно менять занятость карты. Извлечённый юнит и
юнит из списка призыва остаются объектами, но не занимают ячейку. Ссылка на
удалённого юнита становится недействительной.

## Типы юнитов

`wesnoth.unit_types[id]` — неизменяемый справочник. Поля: `id`, `name`,
`alignment`, `race`, `image`, `icon`, `profile`, `small_profile`,
`max_hitpoints`, `max_moves`, `max_experience`, `cost`, `level`, `recall_cost`,
`advances_to`, `advances_from`, `traits`, `abilities`, `ability_ids`, `attacks`,
`variations`, `__cfg`.

`wesnoth.races[id]` содержит данные расы, полы, черты и генераторы имён.

## Атаки

Коллекция `attacks` поддерживает поиск по индексу или id, перебор, добавление,
замену и удаление. Атаки типа юнита неизменяемы. Поля атаки: `read_only`,
`name/id`, `description`, `type`, `icon`, `range`, `alignment`, `damage`,
`number`, `attack_weight`, `defense_weight`, `accuracy`, `parry`,
`movement_used`, `attacks_used`, `specials`, `__cfg`.

## Фильтры и модификации

Фильтр юнита проверяет id, тип, расу, сторону, позицию, состояния, способности,
оружие, переменные и формулы. Поддерживаются вложенные `and`, `or`, `not` и
проверки соседних юнитов. Неизвестные поля WML нужно сохранять без потерь, даже
если конкретная проверка или эффект ещё не реализованы.
