"""Assemble checked English FMG entries from the shared runtime translator output."""
import argparse
import json
import re
import textwrap
from pathlib import Path


def assemble(directory):
    def read(name):
        return json.loads((directory / name).read_text(encoding='utf-8'))
    source = read('fmg-input.json')
    translated = read('fmg-translated.json')
    if len(source) != len(translated):
        raise ValueError('Translation input/output count mismatch')
    lookup = dict(zip(source, translated))
    result = []
    for row in read('fmg-prepared.json'):
        value = ''.join(part if part.startswith('<') else lookup[part]
                        for part in re.split(r'(<[^>]*>)', row['prepared']) if part)
        if re.search('[\u3400-\u9fff]', value):
            raise ValueError(f"Untranslated entry: {row['table']}/{row['id']}")
        if re.findall(r'<[^>]*>', row['text']) != re.findall(r'<[^>]*>', value):
            raise ValueError(f"Markup changed: {row['table']}/{row['id']}")
        if row['table'] == 25 and (100000000 <= row['id'] < 115000000 or 160000000 <= row['id'] < 200000000) and '\n\n' in value:
            lore, info = value.rsplit('\n\n', 1)
            budget = 7 - len(info.splitlines()) - 1
            kept = []
            for paragraph in lore.split('\n\n'):
                lines = textwrap.wrap(' '.join(paragraph.split()), width=72, break_long_words=False, break_on_hyphens=False)
                needed = len(lines) + bool(kept)
                if needed > budget:
                    if not kept and budget > 0:
                        kept.extend(lines[:budget])
                    break
                if kept:
                    kept.append('')
                kept.extend(lines)
                budget -= needed
            value = ('\n'.join(kept) + '\n\n' if kept else '') + info
            if len(value.splitlines()) > 7:
                raise ValueError(f"Weapon caption exceeds seven lines: {row['id']}")
        if re.findall(r'<[^>]*>', row['text']) != re.findall(r'<[^>]*>', value):
            raise ValueError(f"Caption fitting changed markup: {row['table']}/{row['id']}")
        result.append(dict(pack=row['pack'], table=row['table'], id=row['id'], source=row['text'], text=value))
    (directory / 'fmg-english.json').write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
    print(f'Checked {len(result)} English FMG entries: no Chinese, markup retained, weapon captions fitted')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    assemble(parser.parse_args().directory)
