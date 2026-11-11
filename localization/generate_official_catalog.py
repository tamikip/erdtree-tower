"""Retain official English names and prose for runtime item descriptions."""
import argparse
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('directory', type=Path)
args = parser.parse_args()
catalog = json.loads((args.directory / 'official-en.json').read_text(encoding='utf-8'))
variants = json.loads((args.directory / 'official-en-variants.json').read_text(encoding='utf-8'))
for source, choices in variants.items():
    if len(source) > 10 and ('。' in source or '\n' in source):
        catalog[source] = choices[0]
catalog = {source: target for source, target in catalog.items() if not re.search('[\u3400-\u9fff]', target)}
for source, target in list(catalog.items()):
    zh = re.match(r'(?:专用)?战技：([^\n]+)\n', source)
    en = re.match(r'(?:Unique )?Skill: ([^\n]+)\n', target)
    if zh and en:
        catalog[zh[1] + '\n' + source] = en[1] + '\n' + target
target = Path(__file__).with_name('official-en.json')
target.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'Generated {len(catalog)} official English entries')
