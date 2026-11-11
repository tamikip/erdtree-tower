from pathlib import Path
import json
import re

def generate(root):
    root=Path(root)
    catalog={}
    for path in sorted((root/'localization').glob('*-en.json')):
        catalog.update(json.loads(path.read_text(encoding='utf-8')))
    catalog.update(json.loads((root/'localization/en-US.json').read_text(encoding='utf-8')))
    source=(root/'native/native-settings/settings.cpp').read_text(encoding='utf-8')
    values=set()
    for literal in re.findall(r'L"((?:\\.|[^"\\])*)"',source):
        value=json.loads('"'+literal+'"')
        if re.search('[\u3400-\u9fff]',value):values.add(value)
    missing=sorted(values-catalog.keys())
    if missing:raise ValueError('Missing native setting translations: '+repr(missing))
    lines=['#pragma once','#include <string_view>','#include <utility>',
           'inline constexpr std::pair<std::wstring_view,std::wstring_view> tower_english_text[]{']
    for value in sorted(values):
        lines.append('    {L'+json.dumps(value,ensure_ascii=False)+',L'+json.dumps(catalog[value],ensure_ascii=False)+'},')
    lines.append('};')
    target=root/'native/native-settings/translations.h'
    target.write_text('\n'.join(lines)+'\n',encoding='utf-8')
    return len(values)

if __name__=='__main__':print('Generated native settings translations:',generate(Path(__file__).resolve().parents[1]))
