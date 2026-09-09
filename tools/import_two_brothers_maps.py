"""Wrap Battle for Wesnoth's Two Brothers maps for the port.

The original .map files use numeric prefixes (for example ``1 Ke``) to mark
starting locations.  The port places leaders from scenario WML, so those
prefixes are removed while the terrain grid is kept byte-for-byte otherwise.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


START = re.compile(r"^\s*\d+\s+")
PREFIX = re.compile(r"^\d+_")


def convert(source: Path, destination: Path) -> tuple[int, int]:
    rows = [line for line in source.read_text(encoding="utf-8").splitlines() if line.strip()]
    cells = [[START.sub("", cell).strip() for cell in row.split(",")] for row in rows]
    width = len(cells[0])
    if not rows or any(len(row) != width for row in cells):
        raise ValueError(f"non-rectangular map: {source}")

    map_id = PREFIX.sub("", source.stem).lower()
    data = "\n".join(", ".join(row) for row in cells)
    destination.write_text(
        f"[map]\n    id={map_id}\n    width={width}\n    height={len(cells)}\n"
        f"    data=<<\n{data}\n>>\n[/map]\n",
        encoding="utf-8",
    )
    return width, len(cells)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: import_two_brothers_maps.py SOURCE_DIR DESTINATION_DIR")
    source_dir, destination_dir = map(Path, sys.argv[1:])
    destination_dir.mkdir(parents=True, exist_ok=True)
    for source in sorted(source_dir.glob("0[1-4]_*.map")):
        destination = destination_dir / f"{PREFIX.sub('', source.stem).lower()}.wml"
        width, height = convert(source, destination)
        print(f"{source.name}: {width}x{height} -> {destination.name}")


if __name__ == "__main__":
    main()
