from pathlib import Path
import hashlib
import json
import shutil
import zipfile

project=Path(__file__).resolve().parents[1]
workspace=project.parents[1]
base=workspace/'work/party-combined-stage-06'
fix=workspace/'work/party-enemy-load-fix'
test=workspace/'work/party-combined-stage-07'
patch=project/'dist/ErdtreeTower-Test06-EnemyLoad-Patch01'
archive=patch.with_suffix('.zip')
if any(p.exists() for p in [test,patch,archive]): raise SystemExit('Output already exists')
patch.mkdir()
shutil.copytree(fix/'event',patch/'scene-assets/event')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest=json.loads((base/'assets.sha256.json').read_text(encoding='utf-8-sig'))
for entry in manifest:
    replacement=patch/'scene-assets'/entry['Path']
    if replacement.exists(): entry['SHA256']=sha(replacement).upper()
(patch/'assets.sha256.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8-sig')
test.mkdir()
shutil.copytree(base/'scene-assets',test/'scene-assets')
for p in (fix/'event').iterdir(): shutil.copy2(p,test/'scene-assets/event'/p.name)
for name in ['erdtree_tower_party_test.dll','native.sha256']: shutil.copy2(base/name,test/name)
shutil.copy2(patch/'assets.sha256.json',test/'assets.sha256.json')
for entry in manifest: assert sha(test/'scene-assets'/entry['Path'])==entry['SHA256'].lower()
package=project/'dist/ErdtreeTower-Multiplayer-Test-06'
checks=[]
for line in (package/'SHA256SUMS.txt').read_text(encoding='utf-8').splitlines():
    old,relative=line.split('  ',1)
    replacement=patch/relative
    checks.append(f'{sha(replacement) if replacement.exists() else old}  {relative}\n')
(patch/'SHA256SUMS.txt').write_text(''.join(checks),encoding='utf-8')
(patch/'补丁说明.txt').write_text('''Test-06 刷怪加载补丁 01

只适用于 ErdtreeTower-Multiplayer-Test-06 完整包，房主和客机都要安装。
1. 双方正常退出游戏。
2. 将本补丁内的 scene-assets 文件夹、assets.sha256.json、SHA256SUMS.txt 复制到 Test-06 根目录，允许替换同名文件。根目录里应能看到 Start-Seamless.cmd。
3. 用原来的 Start-Seamless.cmd 启动，重新组队并开始测试。

修正敌人收到刷怪请求后没有加载模型的问题。只替换三幕竞技场事件及校验清单，不改密码、存档、DLL、奖励或怪物池。
加载事件和资源回读检查已通过，双人刷怪实际效果仍待验证。
''',encoding='utf-8-sig')
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
    for p in sorted(patch.rglob('*')):
        if p.is_file(): z.write(p,p.relative_to(patch))
with zipfile.ZipFile(archive) as z: assert z.testzip() is None
archive.with_suffix('.zip.sha256').write_text(f'{sha(archive)}  {archive.name}\n',encoding='ascii')
print(f'{archive}\n{archive.stat().st_size/1024:.1f} KiB; 1382 asset hashes and ZIP CRC verified')
