"""Audit exported original/generated parameter tables without launching the game."""
import json
import math
import pathlib
import sys

old_dir, new_dir, catalog = map(pathlib.Path, sys.argv[1:])
tables = ['EquipParamGoods', 'Magic', 'Bullet', 'AtkParam_Pc', 'SpEffectParam']
old = {n: json.loads((old_dir / (n + '.json')).read_text()) for n in tables}
new = {n: json.loads((new_dir / (n + '.json')).read_text()) for n in tables}
values = [[10,18,28,40],[10,18,28,40],[15,25,35,50],[15,25,40,60],
          [15,25,40,55],[20,35,50,75],[15,25,40,60],[10,18,28,40]]
damage = {'atkPhys','atkMag','atkFire','atkThun','atkDark'}
status = {'poizonAttackPower','diseaseAttackPower','bloodAttackPower','freezeAttackPower',
          'sleepAttackPower','madnessAttackPower','curseAttackPower'}

def refs(table, row):
    if table == 'Magic':
        return {f'refId{i}': ['AtkParam_Pc','Bullet','SpEffectParam'][row[f'refCategory{i}']]
                for i in range(1,11) if row[f'refId{i}'] > 0}
    if table == 'Bullet':
        return {'atkId_Bullet':'AtkParam_Pc','HitBulletID':'Bullet','intervalCreateBulletId':'Bullet',
                'spEffectIDForShooter':'SpEffectParam', **{f'spEffectId{i}':'SpEffectParam' for i in range(5)}}
    if table == 'AtkParam_Pc':
        return {f'spEffectId{i}':'SpEffectParam' for i in range(5)}
    if table == 'SpEffectParam':
        return {f:'SpEffectParam' for f in ['replaceSpEffectId','cycleOccurrenceSpEffectId','atkOccurrenceSpEffectId']}
    return {}

for table in tables:
    for id, row in old[table].items():
        assert row == new[table][id], ('original changed', table, id)

count = 0
bullet_origins = {}
for line in catalog.read_text(encoding='utf-8-sig').splitlines():
    _, id, base, tier, name, kind = line.split('|')
    tier, kind = int(tier), int(kind)
    value = values[kind][tier]
    goods = dict(new['EquipParamGoods'][id])
    goods['rarity'] = old['EquipParamGoods'][base]['rarity']
    assert goods == old['EquipParamGoods'][base], ('goods changed', name)
    seen = set()
    changes = []

    def audit(table, src, dst):
        key = (table,src,dst)
        if key in seen:
            return
        seen.add(key)
        if table == 'Bullet':
            assert dst not in bullet_origins or bullet_origins[dst] == src
            bullet_origins[dst] = src
        a, b = old[table][src], new[table][dst]
        links = refs(table,a)
        if kind == 7 and table == 'Bullet':
            assert a['spEffectIDForShooter'] == b['spEffectIDForShooter'], ('caster buildup changed',name)
        if kind == 7 and table == 'Magic':
            for field, target in links.items():
                if target == 'SpEffectParam':
                    assert a[field] == b[field], ('caster spell effect changed',name)
        for field, value_before in a.items():
            value_after = b[field]
            if value_before == value_after:
                continue
            if field in links:
                audit(links[field], str(value_before), str(value_after))
                continue
            expected = None
            if table == 'Magic' and field in ({1:{'mp','mp_charge'},2:{'stamina','stamina_charge'}}.get(kind,set())):
                expected = max(1,math.floor(value_before*(1-value/100)))
            elif table == 'AtkParam_Pc' and (kind in [0,4] and field in damage or kind == 3 and field == 'atkSuperArmor'):
                expected = value_before*(1+value/100)
            elif table == 'SpEffectParam' and (
                kind == 5 and field == 'effectEndurance' and value_before >= 1 or
                kind == 6 and field in {'changeHpPoint','changeHpRate'} and value_before < 0 and a['effectEndurance'] == 0 or
                kind == 7 and field in status and value_before > 0):
                expected = value_before*(1+value/100)
            assert expected is not None, ('unexpected change', name,table,field)
            if field not in {'atkSuperArmor','effectEndurance','changeHpRate'}:
                expected = round(expected)
            assert math.isclose(value_after,expected,rel_tol=1e-6,abs_tol=1e-5), (name,field,value_after,expected)
            changes.append((table,field))

    audit('Magic',base,id)
    assert changes, ('no effective changes',name)
    if kind == 4:
        assert new['Magic'][id]['refId1'] == old['Magic'][base]['refId1'], ('uncharged changed',name)
    count += 1
(catalog.parent / 'spell-affix-bullets.tsv').write_text(''.join(f'{id}\t{bullet_origins[id]}\n' for id in sorted(bullet_origins,key=int)),encoding='utf-8')
print(f'PASS: {count} variants; all original rows unchanged; exact multipliers and isolated references verified')
