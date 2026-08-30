# Контракт платформы

Движок не показывает окна и не читает устройства ввода. Платформа отправляет
команды и получает сообщения с обычными WML-совместимыми данными.

## Входящая команда

```text
Attack {
  attacker: object_id,
  defender: object_id,
  weapon: string
}
```

Команда ничего не знает о мыши, клавиатуре или координатах экрана.

## Исходящие сообщения

`DialogRequested`:

```text
{ dialog, lines: [{ speaker, text }] }
```

`BattleResolved`:

```text
{
  attacker, defender,
  strikes: [{ number, source, target, weapon, chance, roll,
              hit, damage, target_hitpoints }],
  defeated
}
```

`ScenarioFinished`:

```text
{ result: "victory" | "defeat", dialog }
```

Порядок сообщений сохраняется. Визуализатор может анимировать `strikes`, но
анимация не влияет на состояние и не блокирует транзакцию боя.

