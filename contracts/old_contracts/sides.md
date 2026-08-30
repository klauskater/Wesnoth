# ⚠ Контракты сторон

> [!WARNING]
> Устаревший справочный контракт официального Wesnoth. Не реализовывать как API нового движка без отдельного решения.

Стороны — игровое правило Wesnoth, а не свойство универсальной карты. Ядро не
должно знать про золото, найм, врагов, экономику, туман или ИИ.

## Коллекция `wesnoth.sides`

- доступ по номеру, количество и перебор;
- `get(number|save_id)`, `find(filter)`, `matches(side, filter)`, `create(config)`;
- `is_enemy(a, b)`, `set_id(side, id)`;
- настройка ИИ: `append_ai`, `switch_ai`, `debug_ai`, `add_ai_component`,
  `delete_ai_component`, `change_ai_component`;
- туман и пелена: `place_*`, `remove_*`, `override_shroud`, `is_fogged`,
  `is_shrouded`.

## Объект стороны

| Доступ | Поля |
| --- | --- |
| `RW` | `gold`, `objectives`, `village_gold`, `village_support`, `recall_cost`, `base_income`, `objectives_changed`, `fog`, `shroud`, `hidden`, `scroll_to_leader`, `color`, `flag`, `flag_icon`, `user_team_name`, `team_name`, `controller`, `defeat_condition`, `share_vision`, `carryover_bonus`, `carryover_percentage`, `carryover_gold`, `carryover_add`, `lost`, `persistent`, `suppress_end_turn_confirmation`, `side_name`, `shroud_data`, `recruit`, `variables` |
| `R` | `side`, `save_id`, `num_villages`, `total_income`, `faction`, `faction_name`, `is_local`, `share_maps`, `share_view`, `chose_random`, `starting_location`, `num_units`, `total_upkeep`, `expenses`, `net_income`, `__cfg` |

У стороны есть список призыва и набор доступных для найма типов. Владение
деревнями, сторона юнита и туман должны изменяться согласованно. Экономика и
отношения сторон относятся к правилам; локальный игрок, цвет и флаг — к
платформе или игровой сессии.
