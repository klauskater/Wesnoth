# Контракт engine/protocol

Статус: **частично реализован**. Редакция проекта: 1. Общие сообщения, включая
самостоятельный Error, строгий версионированный encode/decode и атомарная
валидация верхнеуровневых блоков готовы. Asset references проверяются против
immutable registry пакета до публикации Snapshot/Update. Базовое дерево UiNode,
уникальность вложенных id, kind/enabled/text/action и его клиентский путь готовы.
SceneItem редакции 1 валидирует ids, anchor/offset, layer/order, frames/timing,
tint, hit id, crop/masks/opacity и asset registry. Layout/style UI и оставшиеся
потребители сцены выполняются в следующих срезах.
Поставщик: `src/engine/protocol.rs`.
[Карточка модуля](../../../../design/architecture/modules/engine/protocol.md).
Обязательны [общие соглашения](../../common.md) и [поток сообщений](../../stream.md).

## Назначение

Общий словарь границы движок–клиент.

## Потребители и зависимости

Потребители: [engine/session](../engine/session.md), [client/connection](../client/connection.md), [client/view](../client/view.md), [client/input](../client/input.md), [client/ui](../client/ui.md), [client/scene](../client/scene.md), [client/effects](../client/effects.md).

Использует контракты: [engine/value](../engine/value.md).
Ссылка Lua → engine означает возможность host API через ctx, а не импорт Rust из Lua.
Обратная ссылка потребителя не разрешает обратный вызов или циклический импорт.

## Операции: вход → выход

```text
validate(message) -> Message | Error
encode/decode(message) -> bytes/Message | Error
```

Это обязательные логические формы, не готовые Rust-сигнатуры. Общие типы определены
в common.md; специфичные типы текущей игры используют её сохранённую семантику,
описанную в [таблице соответствия](../../README.md#прежние-документы).
Конкретный wire layout и имена полей этих игровых записей уточняются в данном
контракте до переключения вызывающих, а не скрыто внутри реализации.

## Гарантии и результат

Структура сообщений и UI/scene определяется только stream.md; проверка не интерпретирует action.

## Ошибки и побочные эффекты

Некорректный тип, версия, повтор id блока или невалидный asset reference отклоняются до применения; получатель не остаётся с частично применённым view.

## Владение данными

Владеет схемой сообщений; игровое содержимое action/payload непрозрачно.

## Что не входит в контракт

HP, время суток, end_turn и названия семейств terrain не являются вариантами Rust enum.

## Приёмка реализации

Сообщение другой игры проходит тот же сериализатор и валидатор.

Перенос из: contracts/platform.md; src/value.rs; src/terrain_scene.rs.
При внедрении добавить ссылку `Контракт: contracts/target/modules/engine/protocol.md`
в заголовок файла поставщика. Статус меняется на «реализован» только вместе
с реализацией, переведёнными потребителями и результатом проверки.
