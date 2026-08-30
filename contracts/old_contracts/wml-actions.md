# Команды и условия WML

> [!WARNING]
> Устаревший справочный контракт официального Wesnoth. Не реализовывать как API нового движка без отдельного решения.

Каждая команда получает один `config`. Неизвестные атрибуты и вложенные теги
должны сохраняться для пользовательских обработчиков. `wml.fire.<name>(cfg)` и
`wesnoth.wml_actions[name](cfg)` обращаются к одному реестру.

## Управление выполнением и переменные

`command`, `continue`, `break`, `for`, `foreach`, `while`, `repeat`, `switch`,
`if`, `then`, `else`, `set_variable`, `set_variables`, `clear_variable`,
`sync_variable`, `store_turns`, `store_map_dimensions`, `store_locations`,
`store_unit`, `store_unit_type`, `store_unit_type_ids`, `store_side`,
`store_gold`, `store_villages`, `store_starting_location`, `store_time_of_day`,
`store_reachable_locations`, `store_relative_direction`,
`store_rotate_map_location`, `test_condition`, `lua`, `unsynced`.

На будущее: `get_global_variable`, `set_global_variable`,
`clear_global_variable`.

## ⚠ Юниты и бой

`unit`, `recall`, `put_to_recall_list`, `unstore_unit`, `kill`, `harm_unit`,
`heal_unit`, `petrify`, `unpetrify`, `transform_unit`, `modify_unit`, `object`,
`remove_object`, `remove_trait`, `role`, `move_unit`, `move_unit_fake`,
`move_units_fake`, `teleport`, `animate_unit`, `unit_overlay`,
`remove_unit_overlay`, `unit_worth`, `store_unit_defense`,
`store_unit_defense_on`.

## ⚠ Карта, стороны и время

`terrain`, `terrain_mask`, `replace_map`, `capture_village`, `tunnel`, `label`,
`time_area`, `remove_time_area`, `replace_schedule`, `gold`, `modify_side`,
`allow_recruit`, `disallow_recruit`, `allow_extra_recruit`,
`disallow_extra_recruit`, `set_recruit`, `set_extra_recruit`, `place_shroud`,
`remove_shroud`, `lift_fog`, `reset_fog`, `modify_turns`.

## ⚠ Сценарий и события

`event`, `remove_event`, `fire_event`, `do_command`, `end_turn`, `endlevel`,
`objectives`, `show_objectives`, `set_menu_item`, `clear_menu_item`,
`allow_undo`, `disallow_undo`, `on_undo`, `allow_end_turn`,
`disallow_end_turn`, `random_placement`, `find_path`.

## ⚠ Изображение, звук и диагностика

`message`, `story`, `item`, `remove_item`, `store_items`, `redraw`, `delay`,
`scroll`, `scroll_to`, `scroll_to_unit`, `lock_view`, `unlock_view`,
`select_unit`, `floating_text`, `color_adjust`, `screen_fade`, `zoom`,
`store_zoom`, `change_theme`, `open_help`, `chat`, `clear_chat`, `print`,
`inspect`, `sound`, `sound_source`, `remove_sound_source`, `music`, `volume`,
`wml_message`, `deprecated_message`.

## ⚠ ИИ и достижения

`modify_ai`, `micro_ai`, `add_ai_behavior`, `set_achievement`,
`set_sub_achievement`, `progress_achievement`.

## Условия

- `have_unit`, `have_location`, `have_side`;
- `variable`, `lua`, `formula`, `test_condition`;
- `and`, `or`, `not`, `true`, `false`;
- `proceed_to_next_scenario`;
- `has_achievement`, `has_sub_achievement`.

Lua-функция, присвоенная в `wesnoth.wml_actions` или
`wesnoth.wml_conditionals`, регистрирует новый пользовательский тег.

Помеченные команды должны оставаться вызываемыми из WML, но исполняться через
правила или платформенный адаптер. Иначе интерпретатор WML незаметно превратится
в монолит всей игры.
