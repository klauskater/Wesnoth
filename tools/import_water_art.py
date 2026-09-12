"""Copy unmodified core water, beach and mask assets used by terrain/water.lua."""
from pathlib import Path
from shutil import copy2

project = Path(__file__).resolve().parents[1]
source = project.parent / "Wesnoth-upstream/data/core/images/terrain"
target = project / "assets/wesnoth/terrain"
patterns = {
    "water": ["water[0-9][0-9].png", "ocean[0-9][0-9].png", "waves-*.png",
              "overlay-*.png", "ford.png", "bottom.png"],
    "sand": ["beach*.png", "desert*.png"],
    "masks": ["long-*.png", "7hex-*.png"],
    "flat": ["bank-to-ice*.png"],
    "swamp": ["*.png"],
}
for folder, globs in patterns.items():
    (target / folder).mkdir(exist_ok=True)
    for pattern in globs:
        for path in (source / folder).glob(pattern):
            copy2(path, target / folder / path.name)

# The decoration renderer uses a flat asset directory.
for path in (source / "embellishments").glob("water-lilies*.png"):
    if not path.stem.endswith("-tile"):
        copy2(path, target / "decorations" / path.name)
