"""Import campaign castle assets unchanged and record available WML variations."""
from pathlib import Path
from shutil import copy2
import re

project = Path(__file__).resolve().parents[1]
source = project.parent / "Wesnoth-upstream/data/core/images/terrain"
target = project / "assets/wesnoth/terrain"
paths = list((source / "castle").glob("*.png"))
for folder in ["elven-ruin", "encampment", "aquatic-castle"]:
    paths += list((source / "castle" / folder).glob("*.png"))
for stem in ["road", "stone-path", "dirt"]:
    paths += [p for p in (source / "flat").glob(stem + "*.png")
              if re.fullmatch(stem + r"\d*", p.stem)]
names = []
paths += [p for p in (source / "flat").glob("road-*.png") if "clean" not in p.stem]
for path in sorted(paths):
    if "tile" in path.stem or "editor" in path.stem:
        continue
    relative = path.relative_to(source)
    (target / relative).parent.mkdir(parents=True, exist_ok=True)
    copy2(path, target / relative)
    names.append(relative.with_suffix("").as_posix())
script = project / "scripts/terrain/castles.lua"
text = script.read_text(encoding="utf-8")
inventory = "-- ASSETS BEGIN\nlocal inventory = [[\n" + "\n".join(names) + "\n]]\n-- ASSETS END"
script.write_text(re.sub(r"-- ASSETS BEGIN.*?-- ASSETS END", lambda _: inventory, text, flags=re.S), encoding="utf-8")
