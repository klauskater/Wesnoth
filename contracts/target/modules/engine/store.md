# Контракт engine/store

Статус: **реализован**. Редакция проекта: 1. Адресные read/select/apply,
атомарные транзакции, optimistic revision, ChangeSet и откат RNG реализованы.
Право commit принадлежит engine/session; карта хранит и возвращает только raw cells.
Поставщик: `src/engine/store.rs`.
[Карточка модуля](../../../../design/architecture/modules/engine/store.md).
Обязательны [общие соглашения](../../common.md) и [поток сообщений](../../stream.md).

## Назначение

Единственное авторитетное хранилище изменяемых данных игры.

## Потребители и зависимости

Потребители: [engine/runtime](../engine/runtime.md), [engine/session](../engine/session.md), [engine/persistence](../engine/persistence.md), [lua/bootstrap](../lua/bootstrap.md), [lua/flow](../lua/flow.md), [lua/units](../lua/units.md), [lua/sides](../lua/sides.md), [lua/movement](../lua/movement.md), [lua/combat](../lua/combat.md), [lua/time](../lua/time.md), [lua/turns](../lua/turns.md), [lua/economy](../lua/economy.md), [lua/visibility](../lua/visibility.md), [lua/scenario](../lua/scenario.md), [lua/campaign](../lua/campaign.md).

Использует контракты: [engine/value](../engine/value.md), [engine/hex](../engine/hex.md).
Ссылка Lua → engine означает возможность host API через ctx, а не импорт Rust из Lua.
Обратная ссылка потребителя не разрешает обратный вызов или циклический импорт.

## Операции: вход → выход

```text
read() -> ReadView
begin(expected_revision) -> Tx
read.get(address) -> Optional<Value>
read.select(addresses, fields) -> Records
tx.apply(operations) -> Unit
tx.random(min,max) -> Int
tx.commit() -> Commit
tx.rollback() -> Unit
```

Это обязательные логические формы, не готовые Rust-сигнатуры. Общие типы определены
в common.md; специфичные типы текущей игры используют её сохранённую семантику,
описанную в [таблице соответствия](../../README.md#прежние-документы).
Конкретный wire layout и имена полей этих игровых записей уточняются в данном
контракте до переключения вызывающих, а не скрыто внутри реализации.

Текущая физическая адресация:

- `objects/<id>` — Value::Map полей объекта без дублирования id;
- `state/<id>` — произвольное Value именованного состояния;
- `map/<x>,<y>` — строковое сырое значение клетки.

Поддерживаемые записи: Insert, Update, Remove и SetField. Для map разрешён только
Update существующей клетки; геометрия карты остаётся фиксированной.

## Гарантии и результат

Address = collection + id; map/state/objects доступны как именованные коллекции. Select возвращает существующие записи в стабильном порядке id; чтение видит собственные записи Tx. Commit содержит revision и изменённые адреса; фиксация доступна только session.

## Ошибки и побочные эффекты

Повтор insert, отсутствующий update/remove, чужая ревизия или истёкшая Tx -> Error. Весь пакет apply проверяется до применения; ошибка команды откатывает все записи и RNG. RNG-изменение также увеличивает ревизию; чистый no-op её не увеличивает.

## Владение данными

Объекты, именованные записи, ревизия мира и RNG; транзакция владеет незавершёнными записями и состоянием RNG.

## Что не входит в контракт

Не интерпретирует игровые поля. Не хранит UI. Не предоставляет прямую изменяемую ссылку на мир клиенту.

## Приёмка реализации

Ошибка после нескольких записей и RNG не меняет ни мир, ни последующую случайную последовательность.

Перенос из: src/engine.rs: World, Object, Transaction, DeterministicRandom.
При внедрении добавить ссылку `Контракт: contracts/target/modules/engine/store.md`
в заголовок файла поставщика. Статус меняется на «реализован» только вместе
с реализацией, переведёнными потребителями и результатом проверки.
