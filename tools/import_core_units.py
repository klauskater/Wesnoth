#!/usr/bin/env python3
"""Convert preprocessed Wesnoth core units into the port's compact WML."""

from __future__ import annotations

import argparse
import importlib.util
import re
import shutil
from pathlib import Path


UNIT_ATTRIBUTES = {
    "id": "id",
    "name": "name",
    "race": "race",
    "image": "image",
    "profile": "profile",
    "hitpoints": "max_hitpoints",
    "movement": "max_moves",
    "experience": "max_experience",
    "level": "level",
    "alignment": "alignment",
    "advances_to": "advances_to",
    "cost": "cost",
    "usage": "usage",
    "movement_type": "movement_type",
    "num_traits": "num_traits",
    "gender": "gender",
    "undead_variation": "undead_variation",
    "zoc": "zoc",
}

TERRAIN_ALIASES = {
    "grassland": "flat",
    "forest": "forest",
    "hills": "hills",
    "water": "shallow_water",
    "castle": "castle",
    "keep": "castle",
    "village": "village",
}

ABILITY_IDS = {
    "healing": "heals",
    "curing": "cures",
    "illumination": "illuminates",
}

SPECIAL_IDS = {
    "drains": "drain",
    "firststrike": "first_strike",
    "petrifies": "petrify",
}


def attributes(node) -> dict[str, str]:
    result = {}
    for attribute in node.get_all(att=""):
        result[attribute.get_name()] = attribute.get_text()
    return result


def child(node, name: str):
    matches = node.get_all(tag=name)
    return matches[-1] if matches else None


def load_parser(path: Path):
    spec = importlib.util.spec_from_file_location("wesnoth_wmlparser3", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load parser: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def scalar(value: str) -> str:
    value = value.replace("\r", "")
    if "\n" in value:
        return "<<\n" + value.replace("\n>>\n", "\n> >\n") + "\n>>"
    return '"' + value.replace("\x00", "") + '"'


def emit_tag(lines: list[str], name: str, values: dict[str, str], children=(), indent=0):
    prefix = " " * indent
    lines.append(f"{prefix}[{name}]")
    for key, value in values.items():
        if value is not None:
            lines.append(f"{prefix}    {key}={scalar(str(value))}")
    for nested in children:
        emit_node(lines, nested, indent + 4)
    lines.append(f"{prefix}[/{name}]")


def emit_node(lines: list[str], node, indent=0):
    emit_tag(lines, node.get_name(), attributes(node), node.get_all(tag=""), indent)


def numeric(value: str, *, invert=False) -> str:
    try:
        number = int(value)
    except (TypeError, ValueError):
        return value
    return str(100 - number if invert else number)


def merged_table(unit, movetype, name: str, *, invert=False) -> dict[str, str]:
    result = attributes(child(movetype, name)) if movetype and child(movetype, name) else {}
    own = child(unit, name)
    if own:
        result.update(attributes(own))
    result = {key: numeric(value, invert=invert) for key, value in result.items()}
    for alias, source in TERRAIN_ALIASES.items():
        if source in result:
            result[alias] = result[source]
    return result


def inherited(unit, units_by_id, tag: str):
    own = unit.get_all(tag=tag)
    if own:
        return own
    base = child(unit, "base_unit")
    if not base:
        return []
    parent = units_by_id.get(base.get_text_val("id"))
    return inherited(parent, units_by_id, tag) if parent else []


def inherited_attribute(unit, units_by_id, name: str):
    value = unit.get_text_val(name)
    if value is not None:
        return value
    base = child(unit, "base_unit")
    parent = units_by_id.get(base.get_text_val("id")) if base else None
    return inherited_attribute(parent, units_by_id, name) if parent else None


def emit_ability(lines: list[str], ability):
    values = attributes(ability)
    source_id = values.get("id", ability.get_name())
    values["id"] = ABILITY_IDS.get(source_id, source_id)
    values["source_id"] = source_id
    values["kind"] = ability.get_name()
    if "value" not in values and "heal_amount" in values:
        values["value"] = values["heal_amount"]
    emit_tag(lines, "ability", values, ability.get_all(tag=""), 4)


def emit_attack(lines: list[str], attack):
    source = attributes(attack)
    attack_id = source.get("name", "attack")
    values = {
        "id": attack_id,
        "name": source.get("description", attack_id),
        "range": source.get("range", "melee"),
        "damage_type": source.get("type", "blade"),
        "damage": source.get("damage", "0"),
        "strikes": source.get("number", "1"),
    }
    for name in ("accuracy", "parry", "attack_weight", "defense_weight", "movement_used",
                 "attacks_used", "min_range", "max_range", "alignment", "icon"):
        if name in source:
            values[name] = source[name]
    lines.append("    [attack]")
    for key, value in values.items():
        lines.append(f"        {key}={scalar(value)}")
    for specials in attack.get_all(tag="specials"):
        for special in specials.get_all(tag=""):
            special_values = attributes(special)
            source_id = special_values.get("id", special.get_name())
            normalized = re.sub(r"\(.*\)$", "", source_id)
            special_values["id"] = SPECIAL_IDS.get(normalized, normalized)
            special_values["source_id"] = source_id
            special_values["kind"] = special.get_name()
            emit_tag(lines, "special", special_values, special.get_all(tag=""), 8)
    lines.append("    [/attack]")


def emit_race(lines: list[str], race):
    values = attributes(race)
    kept = {
        key: values[key]
        for key in ("id", "name", "plural_name", "description", "num_traits", "ignore_global_traits")
        if key in values
    }
    emit_tag(lines, "race", kept, race.get_all(tag="trait"))


def emit_unit(lines: list[str], unit, units_by_id, movetypes):
    values = {}
    for source, target in UNIT_ATTRIBUTES.items():
        value = inherited_attribute(unit, units_by_id, source)
        if value is not None:
            values[target] = value
    movement_type = movetypes.get(values.get("movement_type"))
    lines.append("[unit_type]")
    for key, value in values.items():
        lines.append(f"    {key}={scalar(value)}")
    emit_tag(lines, "movement_costs", merged_table(unit, movement_type, "movement_costs"), indent=4)
    emit_tag(lines, "defense", merged_table(unit, movement_type, "defense", invert=True), indent=4)
    emit_tag(lines, "resistance", merged_table(unit, movement_type, "resistance", invert=True), indent=4)
    for trait in inherited(unit, units_by_id, "trait"):
        emit_node(lines, trait, 4)
    for box in inherited(unit, units_by_id, "abilities"):
        for ability in box.get_all(tag=""):
            emit_ability(lines, ability)
    for attack in inherited(unit, units_by_id, "attack"):
        emit_attack(lines, attack)
    for advancement in inherited(unit, units_by_id, "advancement"):
        emit_node(lines, advancement, 4)
    for variant_name in ("female", "male", "variation"):
        for variant in unit.get_all(tag=variant_name):
            emit_node(lines, variant, 4)
    lines.append("[/unit_type]")


def copy_images(units, units_by_id, image_root: Path, output: Path, index: Path):
    entries = []
    for unit in units:
        unit_id = unit.get_text_val("id")
        image = inherited_attribute(unit, units_by_id, "image")
        if not image:
            continue
        relative = image.split("~", 1)[0]
        source = image_root / relative
        if not source.is_file():
            continue
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        entries.append(f"{unit_id}\t{relative.replace('\\', '/')}")
    index.parent.mkdir(parents=True, exist_ok=True)
    index.write_text("\n".join(entries) + "\n", encoding="utf-8")
    return len(entries)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plain", type=Path, help="units.cfg.plain produced by wesnoth --preprocess")
    parser.add_argument("--wmlparser", required=True, type=Path, help="path to upstream wmlparser3.py")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--image-root", type=Path, help="directory containing the upstream units/ images")
    parser.add_argument("--image-output", type=Path)
    parser.add_argument("--image-index", type=Path)
    args = parser.parse_args()

    wml = load_parser(args.wmlparser)
    root = wml.Parser().parse_file(str(args.plain))
    containers = root.get_all(tag="units")
    if len(containers) != 1:
        raise RuntimeError(f"expected one [units] root, found {len(containers)}")
    container = containers[0]
    units = container.get_all(tag="unit_type")
    races = container.get_all(tag="race")
    global_traits = container.get_all(tag="trait")
    movetypes = {node.get_text_val("name"): node for node in container.get_all(tag="movetype")}
    units_by_id = {node.get_text_val("id"): node for node in units}

    lines = [
        "# Generated by tools/import_core_units.py; do not edit by hand.",
        f"# Source: {args.plain.name} ({len(races)} races, {len(units)} unit types).",
        "",
    ]
    for trait in global_traits:
        emit_node(lines, trait)
        lines.append("")
    for race in races:
        emit_race(lines, race)
        lines.append("")
    for unit in units:
        emit_unit(lines, unit, units_by_id, movetypes)
        lines.append("")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(lines), encoding="utf-8")

    image_count = 0
    if args.image_root and args.image_output and args.image_index:
        image_count = copy_images(
            units, units_by_id, args.image_root, args.image_output, args.image_index
        )
    print(
        f"wrote {len(global_traits)} global traits, {len(races)} races, "
        f"{len(units)} unit types, {image_count} images"
    )


if __name__ == "__main__":
    main()
