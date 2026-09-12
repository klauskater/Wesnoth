"""Copy original wooden and basic stone bridge sprites without resizing."""
from pathlib import Path
from shutil import copy2

project = Path(__file__).resolve().parents[1]
source = project.parent / "Wesnoth-upstream/data/core/images/terrain/bridge"
target = project / "assets/wesnoth/terrain/bridge"
target.mkdir(exist_ok=True)
for pattern in ("wood-*.png", "stonebridge*.png"):
    for path in source.glob(pattern):
        if "rotting" not in path.name and "tile" not in path.name:
            copy2(path, target / path.name)
