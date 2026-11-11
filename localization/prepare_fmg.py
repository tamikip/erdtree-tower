from pathlib import Path
import json,re

import argparse
parser=argparse.ArgumentParser()
parser.add_argument('directory',type=Path)
parser.add_argument('--assets',type=Path,default=Path(__file__).resolve().parents[1]/'scene-assets')
args=parser.parse_args()
out=args.directory
official=json.loads((out/'official-en.json').read_text(encoding='utf-8'))
variants=json.loads((out/'official-en-variants.json').read_text(encoding='utf-8'))
for source,choices in variants.items():
    if len(source)>80 and '\n' in source:
        official[source]=choices[0]
paragraphs={}
for source,target in official.items():
    left=re.split(r'\n\s*\n',source.strip())
    right=re.split(r'\n\s*\n',target.strip())
    if len(left)!=len(right):continue
    for zh,en in zip(left,right):
        if len(zh)>=6 and re.search('[\u3400-\u9fff]',zh) and not re.search('[\u3400-\u9fff]',en):
            paragraphs.setdefault(zh,set()).add(en)
official.update({zh:next(iter(values)) for zh,values in paragraphs.items() if len(values)==1 and zh not in official})
trie={}
for source,target in official.items():
    if len(source)<12 or not re.search('[\u3400-\u9fff]',source) or re.search('[\u3400-\u9fff]',target):continue
    node=trie
    for char in source:node=node.setdefault(char,{})
    node[None]=target
def vanilla(text):
    if text in official and not re.search('[\u3400-\u9fff]',official[text]):return official[text]
    parts=re.split(r'(\n\s*\n)',text)
    if len(parts)>1:
        return ''.join(vanilla(part) if index%2==0 else part for index,part in enumerate(parts))
    result=[];i=0
    while i<len(text):
        node=trie;j=i;found=None
        while j<len(text) and text[j] in node:
            node=node[text[j]];j+=1
            if None in node:found=(j,node[None])
        if found:i,value=found;result.append(value)
        else:result.append(text[i]);i+=1
    return ''.join(result)
rows=json.loads((out/'custom-fmg.json').read_text(encoding='utf-8'))
context={(row['pack'],row['table'],row['id']):row for row in json.loads((out/'official-context.json').read_text(encoding='utf-8'))}
assets=args.assets
identities={}
for name in ('affix-catalog.txt','affix-v2-catalog.txt'):
    for line in (assets/name).read_text(encoding='utf-8').splitlines():
        cells=line.split('|')
        if cells[0] in ('weapon','armor'):
            identities[(25 if cells[0]=='weapon' else 26,int(cells[1]))]=int(cells[2])
def prepare(row):
    table=row['table'];item=row['id']//10000*10000 if table==25 else row['id']
    base=identities.get((table,item))
    original=context.get((row['pack'],table,base))
    if original and '\n\n' in row['text']:
        lore,info=row['text'].rsplit('\n\n',1)
        if lore and original['source'].startswith(lore):
            return original['text']+'\n\n'+info
    return vanilla(row['text'])
unique={}
for row in rows:
    # Original English prose is retained; only mod additions go through the mod catalog.
    value=prepare(row)
    for part in re.split(r'(<[^>]*>)',value):
        if part and not part.startswith('<'):unique.setdefault(part,len(unique))
(out/'fmg-input.json').write_text(json.dumps(list(unique),ensure_ascii=False),encoding='utf-8')
(out/'fmg-prepared.json').write_text(json.dumps([dict(row,prepared=prepare(row)) for row in rows],ensure_ascii=False),encoding='utf-8')
print('Unique mod text fragments:',len(unique))
