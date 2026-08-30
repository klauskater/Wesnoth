# ⚠ Совместимость и вспомогательные режимы

> [!WARNING]
> Устаревший справочный контракт официального Wesnoth. Не реализовывать как API нового движка без отдельного решения.

Эти имена встречаются в официальных скриптах, но являются старыми обёртками над
основным API. Их можно реализовать на Lua. Дублировать каждую функцию внутри
движка не требуется.

Опасность: старые глобальные функции обходят аккуратные границы модулей. Их
лучше оставить тонкими Lua-обёртками и не добавлять как нативные методы ядра.

## Юниты

- `get_unit` -> `units.get`; `get_units` -> `units.find_on_map`;
- `get_recall_units` -> `units.find_on_recall`; `create_unit` -> `units.create`;
- `put_unit` -> `units.to_map`; `put_recall_unit` -> `units.to_recall`;
- `erase_unit`, `extract_unit`, `copy_unit`, `advance_unit`, `transform_unit`,
  `teleport`, `select_unit` -> одноимённые операции `units.*`;
- `match_unit`, `add_modification`, `remove_modifications`;
- `unit_ability`, `unit_defense`, `unit_movement_cost`, `unit_vision_cost`,
  `unit_jamming_cost`, `unit_resistance` -> расчёты `units.*`;
- `create_animator`, `create_weapon`, `get_displayed_unit`.

## Карта, пути и время

- `get_locations`/`match_location`, `get_terrain`/`set_terrain`,
  `get_terrain_info`, `get_map_size`, `create_map`, `terrain_mask`;
- `get_village_owner`/`set_village_owner`, `label`, `add_time_area`,
  `remove_time_area`, `special_locations`;
- старые имена геометрии: `tiles_adjacent`, `vector_sum`, `vector_diff`,
  `vector_negation`, `get_tiles_radius` и опечатка `hex_vector_dif`;
- `find_path`, `find_reach`, `find_cost_map`, `find_vacant_tile`,
  `find_vision_range` -> `paths.*`;
- `get_time_of_day`, `replace_schedule`, `set_time_of_day` -> `schedule.*`.

## Стороны, сценарий и события

- `get_sides`, `create_side`, `match_side`, `is_enemy`, `set_side_id`;
- старые функции тумана, пелены и ИИ соответствуют `sides.*`;
- `get_side_variable`, `set_side_variable`, `get_starting_location` работают
  через объект стороны;
- `set_next_scenario` и настройки финального текста/музыки записывают поля
  `wesnoth.scenario`;
- `fire_event`, `fire_event_by_id`, `add_event_handler`,
  `remove_event_handler`, `allow_undo`, `invoke_synced_command`,
  `synchronize_choice(s)`, `unsynced` соответствуют `game_events` и `sync`.

## Интерфейс, GUI и звук

Старые глобальные функции задержки, перерисовки, камеры, выделения, сообщений,
меню и конца хода перенаправляются в `wesnoth.interface`. Старые функции окон
перенаправляются в `gui`, звука и музыки — в `wesnoth.audio`.

## Общие помощники

`read_file`, `have_file`, `canonical_path`, `get_image_size` -> `filesystem`;
`format*` -> `stringx`; `random` -> синхронизированный `mathx.random`;
`debug`, `tovconfig`, `wml_matches_filter`, `eval_conditional`, `get_variable`,
`set_variable`, `get_all_vars` -> модуль `wml`.

`wesnoth.game_config` предоставляет только для чтения: `base_income`,
`village_income`, `village_support`, `poison_amount`, `rest_heal_amount`,
`recall_cost`, `kill_experience`, `combat_experience`, `debug`, `debug_lua`,
`strict_lua`, `mp_debug`, палитры и цветовые шкалы.

## ⚠ Генератор карт

Отдельный режим Lua для генерации карт требует создания и изменения автономной
карты, фильтров местности, `wesnoth.map.generate`, `generate_height_map`,
генераторов стандартной карты и высот, поиска пути и разбора WML. У него нет
доступа к юнитам, сторонам и событиям активного сценария.

## ⚠ Плагины и прикладной Lua

Официальный Wesnoth также имеет корутинные помощники `next_slice`,
`wait_until*`, GUI и файловые функции для плагинов. Они не относятся к
безголовому игровому движку и пока не входят в порт.

## ⚠ Сохранение состояния

`game_events.on_load`, `game_events.on_save`, `persistent_tags` и глобальные
переменные оставлены как будущие точки расширения. Пока сохранений нет, система
должна честно сообщать, что они не поддерживаются, а не молча терять данные.
