# Контракт engine/runtime

Статус: **реализован**. Редакция проекта: 1. Постоянная sandboxed VM, package
require, отдельные write/read entry-входы, лимит инструкций и истечение ctx
реализованы; Store и commit принадлежат Session. Runtime сохраняет тип ошибки
лимита, поэтому Session выдаёт `budget_exceeded`, а не общий `script_error`.
Поставщик: `src/engine/runtime.rs`.
[Карточка модуля](../../../../design/architecture/modules/engine/runtime.md).
Обязательны [общие соглашения](../../common.md) и [поток сообщений](../../stream.md).

## Назначение

Исполнение модулей Lua с ограниченными возможностями доступа.

## Потребители и зависимости

Потребители: [engine/session](../engine/session.md).

Использует контракты: [engine/store](../engine/store.md), [engine/resources](../engine/resources.md), [engine/hex](../engine/hex.md), [engine/pathfinding](../engine/pathfinding.md), [engine/value](../engine/value.md), [lua/entry](../lua/entry.md).
Ссылка Lua → engine означает возможность host API через ctx, а не импорт Rust из Lua.
Обратная ссылка потребителя не разрешает обратный вызов или циклический импорт.

## Операции: вход → выход

```text
load_entry(package) -> Entry
invoke_initialize(tx, request) -> Unit
invoke_dispatch(tx, command) -> DomainEvents
invoke_present(read, viewer, view_context, changes, events) -> Presentation
invoke_interact(read,viewer,view_context,input) -> InteractionResult
```

Это обязательные логические формы, не готовые Rust-сигнатуры. Общие типы определены
в common.md; специфичные типы текущей игры используют её сохранённую семантику,
описанную в [таблице соответствия](../../README.md#прежние-документы).
Конкретный wire layout и имена полей этих игровых записей уточняются в данном
контракте до переключения вызывающих, а не скрыто внутри реализации.

## Гарантии и результат

Tx/read — ограниченные возможности, не копия World. Require внутри пакета, один экспорт на модуль в VM. Возвращённые данные проверяются до commit; present не пишет мир и не использует игровой RNG.

## Ошибки и побочные эффекты

Ошибка Lua, цикла импортов, лимита выполнения, неверного результата -> Error; истёкшие ctx отклоняются. Rollback Rust не откатывает Lua globals: хранить там авторитетные данные запрещено.

## Владение данными

Одна постоянная VM на сессию; кеш загруженных модулей, но не второе игровое состояние.

## Что не входит в контракт

Нет произвольного доступа к файлам/сети; require только внутри пакета. Нет отдельной VM или загрузки исходников на каждый клик.

## Приёмка реализации

Повторный импорт возвращает тот же модуль; циклический импорт выдаёт понятную ошибку; present не может записывать данные.

Перенос из: src/engine.rs: Lua/evaluate/create_context; склейка rules в src/game.rs.
При внедрении добавить ссылку `Контракт: contracts/target/modules/engine/runtime.md`
в заголовок файла поставщика. Статус меняется на «реализован» только вместе
с реализацией, переведёнными потребителями и результатом проверки.
