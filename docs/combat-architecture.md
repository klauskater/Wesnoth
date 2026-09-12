# Combat boundary

Lua `prepare_battle` prepares the same weapon parameters for `preview_attack` and `resolve`: retaliation, bonuses, modified damage, slowed damage and strike order. Parameters remain fixed during this exchange except that applying slow selects the prepared alternate damage. Lua retains status effects, experience, death, advancement and scenario rules.

Rust `context.combat:forecast` receives two compact participants once. It merges states by both HP values, slowed flags, combat termination and whether either participant has been hit. It returns marginal HP distributions, expected HP, death probability and probability of no hits. Forecasts do not consume RNG or mutate the world. The arithmetic of damage and healing is shared with actual strikes through `context.combat:impact`; the healing percentage comes from Lua. Actual strike calls transfer scalar values, never unit objects. The pre-existing berserk behavior (30 exchanges) is preserved.

The client queries available actions and previews when opening the weapon dialog, caches them for the world revision, and sends only attacker, defender and weapon IDs to `resolve`. The command revalidates the action. Strike events include both post-strike HP values and weapon/range. Client playback retains the pre-command snapshot so deaths and HP changes appear at impact, rather than exposing the final snapshot immediately.

`tools/import_battle_art.py` imports attack frames from preprocessed upstream WML. The manifest selects by unit type, weapon, direction and hit/miss. Textures are uploaded only for battle participants. This initial importer uses the first matching animation and stretches its frames across a fixed strike duration. It does not yet reproduce frame-specific timing, projectile subanimations, sound, arbitrary animation filters or defender/death animations. Player-initiated battles are animated; the existing end-turn AI command still completes as one batch.

Validation: screenshot archer example (78.4% death, 21.6% no hits), forecast normalization and RNG immutability, real outcomes within forecast support, and existing combat special/AI/transaction tests.
