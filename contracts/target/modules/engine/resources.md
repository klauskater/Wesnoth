# Контракт engine/resources

Статус: **реализован**. Редакция проекта: 1. Manifest, безопасные логические пути,
text/data/module, проверяемый реестр AssetDescriptor и одинаковая загрузка
disk/embedded реализованы.
Поставщик: `src/engine/resources.rs`.
[Карточка модуля](../../../../design/architecture/modules/engine/resources.md).
Обязательны [общие соглашения](../../common.md) и [поток сообщений](../../stream.md).

## Назначение

Доступ к содержимому сменяемого пакета игры.

## Потребители и зависимости

Потребители: [engine/runtime](../engine/runtime.md), [engine/session](../engine/session.md), [engine/persistence](../engine/persistence.md), [lua/bootstrap](../lua/bootstrap.md), [lua/terrain](../lua/terrain.md), [lua/campaign](../lua/campaign.md), [lua/terrain_rendering](../lua/terrain_rendering.md), [lua/effects](../lua/effects.md).

Использует контракты: [engine/data_format](../engine/data_format.md).
Ссылка Lua → engine означает возможность host API через ctx, а не импорт Rust из Lua.
Обратная ссылка потребителя не разрешает обратный вызов или циклический импорт.

## Операции: вход → выход

```text
open(manifest) -> Package
read_text(path) -> String
read_data(path) -> DataNode
asset(id) -> AssetDescriptor
module(name) -> Source
```

Это обязательные логические формы, не готовые Rust-сигнатуры. Общие типы определены
в common.md; специфичные типы текущей игры используют её сохранённую семантику,
описанную в [таблице соответствия](../../README.md#прежние-документы).
Конкретный wire layout и имена полей этих игровых записей уточняются в данном
контракте до переключения вызывающих, а не скрыто внутри реализации.

## Гарантии и результат

Manifest содержит package_id, package_version, entry, resource_roots, protocol_version. Ресурсы неизменяемы на время сессии; disk/embedded имеют одинаковые логические пути.

## Ошибки и побочные эффекты

Неизвестный asset/module, выход из корня пакета, неверный manifest -> Error. Обновление пакета требует новой сессии, не меняет данные под активным кешем.

## Владение данными

Неизменяемые ресурсы пакета; manifest содержит package_id, version, entry, resource roots, protocol_version.

## Что не входит в контракт

Не сливает unit_type/race/trait и не ищет деревни. Не компилирует сценарные правила в Rust.

## Приёмка реализации

Один пакет одинаково читается с диска и из embedded; путь за пределы пакета отклоняется.

Перенос из: src/embedded.rs; загрузка файлов в src/game.rs; build.rs.
При внедрении добавить ссылку `Контракт: contracts/target/modules/engine/resources.md`
в заголовок файла поставщика. Статус меняется на «реализован» только вместе
с реализацией, переведёнными потребителями и результатом проверки.
