from pathlib import Path
import shutil
import sys
import xml.etree.ElementTree as ET
from PIL import Image

source, target = map(Path, sys.argv[1:])
rows = [line.split('\t') for line in (source / 'reward-details.tsv').read_text(encoding='utf-8-sig').splitlines()]
needed = {int(row[1]) for row in rows}
destination = target / 'reward-icons'
destination.mkdir(parents=True, exist_ok=True)
found = set()
for layout in source.glob('SB_Icon*.layout'):
    entries = {int(e.attrib['name'].removeprefix('MENU_ItemIcon_').removesuffix('.png')): e.attrib
               for e in ET.parse(layout).getroot() if e.attrib['name'].startswith('MENU_ItemIcon_')}
    relevant = needed.intersection(entries)
    if not relevant:
        continue
    with Image.open(layout.with_suffix('.dds')) as atlas:
        for icon in relevant:
            entry = entries[icon]
            x, y, w, h = (int(entry[k]) for k in ('x', 'y', 'width', 'height'))
            image = atlas.crop((x, y, x+w, y+h)).convert('RGBA').resize((160, 160), Image.Resampling.LANCZOS)
            (destination / f'{icon}.rgba').write_bytes(image.tobytes())
            found.add(icon)
assert found == needed, f'Missing icons: {needed-found}'
assert len({row[0] for row in rows}) == len(rows)
assert all(len(row) == 7 and row[2] and row[4] for row in rows)
shutil.copyfile(source / 'reward-details.tsv', target / 'reward-details.tsv')
print(f'Validated {len(rows)} descriptions and {len(found)} native icons')
