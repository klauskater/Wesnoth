"""Import first directional standing frames from preprocessed core units."""
import argparse
import re
import shutil
from pathlib import Path
from import_core_units import load_parser, inherited


def import_art(plain, upstream, output):
    parser = load_parser(upstream / "data/tools/wesnoth/wmlparser3.py")
    units = parser.Parser().parse_file(str(plain)).get_all(tag="units")[0].get_all(tag="unit_type")
    by_id = {u.get_text_val("id"): u for u in units}
    entries = {}
    def visit(node, directions=""):
        directions = node.get_text_val("direction") or directions
        for frame in node.get_all(tag="frame"):
            image = frame.get_text_val("image") or ""
            image = re.sub(r"\[([^]~,*]+)[^]]*\]", r"\1", image).split(":")[0].split("~")[0]
            if image and (upstream / "data/core/images" / image).is_file():
                for direction in directions.split(","):
                    if direction.strip() in ("n", "ne", "se", "s", "sw", "nw"):
                        entries.setdefault((unit_id, direction.strip()), image)
                break
        for tag in ("if", "else"):
            for branch in node.get_all(tag=tag):
                visit(branch, directions)
    for unit in units:
        unit_id = unit.get_text_val("id")
        for animation in inherited(unit, by_id, "standing_anim"):
            visit(animation)
    for image in set(entries.values()):
        target = output / image
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            shutil.copy2(upstream / "data/core/images" / image, target)
    (output / "directions.tsv").write_text("".join(f"{unit}:{direction}\t{image}\n" for (unit, direction), image in sorted(entries.items())), encoding="utf-8")
    footsteps = output.parent / "footsteps"
    footsteps.mkdir(exist_ok=True)
    for image in (upstream / "images/footsteps/default").glob("*.png"):
        shutil.copy2(image, footsteps / image.name)
    print(f"Imported {len(entries)} directional entries")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("plain", type=Path)
    parser.add_argument("upstream", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    import_art(args.plain, args.upstream, args.output)
