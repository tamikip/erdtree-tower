import csv
import json
from pathlib import Path
import yaml

root = Path('outputs/erdtree-tower')
enemies = yaml.safe_load(Path('work/SoulsRandomizers/diste/Base/enemy.txt').read_text(encoding='utf-8'))['Enemies']
raw = json.loads(Path('work/boss-report/raw.json').read_text())
excluded = set('''Patches|Tibia Mariner Boss|Night's Cavalry|Godrick the Grafted|Ancestor Spirit|Adan, Thief of Fire|Crucible Spiritcaller Snail|Rennala 1|Rennala 2|Commander O'Neil|Starscourge Radahn|Mimic Tear|Regal Ancestor Spirit|Invisible Black Knife Assassin Boss|Necromancer Garris|Fia's Champions|God-Devouring Serpent|Rykard, Lord of Blasphemy|Esgar, Priest of Blood|Morgott, the Omen King|Erdtree Avatar Duo|Fire Giant 1|Fire Giant 2|Godskin Spiritcaller Snail|Roundtable Knight Vyke|Commander Niall|Beast Clergyman|Godfrey, First Elden Lord|Godskin Duo|Hoarah Loux, Warrior|Maliketh, the Black Blade|Sir Gideon Ofnir, the All-Knowing|Stray Mimic Tear|Dragonlord Placidusax|Mohg, Lord of Blood|Elden Beast|Radagon of the Golden Order|Malenia, Blade of Miquella'''.split('|'))
large = set('''Flying Dragon Agheel|Glintstone Dragon Smarag|Glintstone Dragon Adula|Decaying Ekzykes|Ancient Dragon Lansseax|Astel, Naturalborn of the Void|Lichdragon Fortissax|Flying Dragon Greyll|Borealis the Freezing Fog|Astel, Stars of Darkness'''.split('|'))
records, coverage = [], []
excluded.add('Dragonkin Soldier of Nokstella')
for row in csv.DictReader((root/'reference/本体Boss等级与抗性.csv').open(encoding='utf-8-sig')):
    values = list(row.values())
    name, key = values[0], values[-1]
    hits = [r for r in raw if r['s']['Group'] == key] or [r for r in raw if r['s']['Key'] == key]
    assert hits, key
    if key in excluded | large:
        coverage.append(dict(name=name, key=key, status='excluded', reason='需要专属大型场地' if key in large else '专属阶段、召唤或角色机制'))
        continue
    source = hits[0]['s']
    parent = next(e for e in enemies if e['ID'] == source['ID'])
    members = [parent] + [e for e in enemies if e.get('OwnedBy') == source['ID'] and e.get('Class') == 'Helper' and 'multi' in e.get('Tags', '')]
    members.sort(key=lambda e: e is not parent and 'mainhelper' not in e.get('Tags', ''))
    assert len(members) <= 12, key
    act = 1 if int(values[2]) < 50 else 2 if int(values[2]) < 90 else 3
    records.append(dict(name=name, key=key, act=act, members=[dict(map=e['Map'], name=e['Name'], entity=e['ID'], major=e is parent or 'mainhelper' in e.get('Tags', '')) for e in members]))
    coverage.append(dict(name=name, key=key, status='included', act=act, members=len(members)))
Path('work/boss-candidates.json').write_text(json.dumps(records, ensure_ascii=False), encoding='utf-8')
Path('work/boss-coverage.json').write_text(json.dumps(coverage, ensure_ascii=False, indent=2), encoding='utf-8')
print(f'{len(records)} encounters, {len(coverage)-len(records)} excluded; {sum(len(r["members"]) for r in records)} components')
