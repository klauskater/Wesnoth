"""Import directional weapon attack frames from preprocessed original unit definitions."""
import argparse, json, re, shutil
from pathlib import Path
from import_core_units import load_parser, inherited

def expand(image):
    match = re.search(r"\[([^]]+)\]", image)
    if not match: return [image]
    result=[]
    for part in match[1].split(","):
        if "~" in part:
            a,b=map(int,part.split("~")); values=range(a,b+(1 if b>=a else -1),1 if b>=a else -1)
        else: values=[part]
        for value in values:
            result.extend(expand(image[:match.start()]+str(value)+image[match.end():]))
    return result

def main(plain, upstream, output):
    parser=load_parser(upstream / "data/tools/wesnoth/wmlparser3.py")
    units=parser.Parser().parse_file(str(plain)).get_all(tag="units")[0].get_all(tag="unit_type")
    by_id={u.get_text_val("id"):u for u in units}
    entries={}; images=set()
    for unit in units:
        uid=unit.get_text_val("id")
        weapons=[a.get_text_val("name") for a in inherited(unit,by_id,"attack")]
        for anim in inherited(unit,by_id,"attack_anim"):
            filters=anim.get_all(tag="filter_attack")
            selected=weapons
            if filters:
                name=filters[0].get_text_val("name")
                selected=name.split(",") if name else [a.get_text_val("name") for a in inherited(unit,by_id,"attack") if not filters[0].get_text_val("range") or a.get_text_val("range")==filters[0].get_text_val("range")]
            def visit(node, directions="n,ne,se,s,sw,nw", hits="yes,no", prefix=None):
                directions=node.get_text_val("direction") or directions
                hits=node.get_text_val("hits") or hits
                frames=list(prefix or [])
                for frame in node.get_all(tag="frame"):
                    image=frame.get_text_val("image") or ""
                    image=image.split(":")[0].split("~FL")[0]
                    try: expanded=expand(image)
                    except ValueError: continue
                    for image in expanded:
                        if "~" not in image and (upstream/"data/core/images"/image).is_file():frames.append(image)
                for branch in node.get_all(tag="if")+node.get_all(tag="else"):
                    visit(branch,directions,hits,frames)
                if frames:
                    for weapon in selected:
                        for direction in directions.split(","):
                            for hit in hits.split(","):
                                key=f"{uid}|{weapon.strip()}|{direction.strip()}|{hit.strip()}"
                                if key not in entries: entries[key]=frames; images.update(frames)
            visit(anim)
    output.mkdir(parents=True,exist_ok=True)
    for image in sorted(images):
        target=output/image;target.parent.mkdir(parents=True,exist_ok=True)
        if not target.exists():shutil.copy2(upstream/"data/core/images"/image,target)
    (output/"animations.json").write_text(json.dumps(entries,ensure_ascii=False),encoding="utf-8")
    (output/"index.tsv").write_text("".join(f"{p}\t{p}\n" for p in sorted(images)),encoding="utf-8")
    print(f"{len(entries)} animation variants, {len(images)} frames")
if __name__=="__main__":
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("plain",type=Path);p.add_argument("upstream",type=Path);p.add_argument("output",type=Path)
    a=p.parse_args();main(a.plain,a.upstream,a.output)
