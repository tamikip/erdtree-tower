from pathlib import Path
import hashlib
import json
import shutil
import zipfile
from native_build import build_native

project=Path(__file__).resolve().parents[1]
workspace=project.parents[1]
base=workspace/'work/party-combined-stage-07'
fix=workspace/'work/party-completion-fix'
stage=workspace/'work/party-combined-stage-08'
patch=project/'dist/ErdtreeTower-Test06-Completion-Patch02'
archive=patch.with_suffix('.zip')
if any(p.exists() for p in [stage,patch,archive]): raise SystemExit('Output already exists')
built_native = build_native(project)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
patch.mkdir()
shutil.copytree(fix,patch/'scene-assets')
shutil.copy2(built_native,patch/'erdtree_tower_party_test.dll')
(patch/'native.sha256').write_text(sha(patch/'erdtree_tower_party_test.dll').upper()+'\n',encoding='ascii')
manifest=json.loads((base/'assets.sha256.json').read_text(encoding='utf-8-sig'))
for entry in manifest:
    replacement=patch/'scene-assets'/entry['Path']
    if replacement.exists(): entry['SHA256']=sha(replacement).upper()
(patch/'assets.sha256.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8-sig')
stage.mkdir()
shutil.copytree(base/'scene-assets',stage/'scene-assets')
for p in patch.rglob('*'):
    if p.is_file():
        dest=stage/p.relative_to(patch);dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,dest)
for entry in manifest: assert sha(stage/'scene-assets'/entry['Path'])==entry['SHA256'].lower()
assert sha(stage/'erdtree_tower_party_test.dll').upper()==(stage/'native.sha256').read_text().strip()
package=project/'dist/ErdtreeTower-Multiplayer-Test-06'
checks=[]
for line in (package/'SHA256SUMS.txt').read_text(encoding='utf-8').splitlines():
    old,relative=line.split('  ',1);replacement=patch/relative
    checks.append(f'{sha(replacement) if replacement.exists() else old}  {relative}\n')
(patch/'SHA256SUMS.txt').write_text(''.join(checks),encoding='utf-8')
(patch/'补丁说明.txt').write_text('''Test-06 联机结算与粪金龟测试补丁 02

房主与所有队友都必须更新。适用于 Test-06，包含补丁 01 的刷怪事件修复。
1. 双方正常退出游戏。
2. 将本补丁全部内容复制到 Test-06 根目录，替换同名文件；根目录应有 Start-Seamless.cmd。
3. 用原来的 Start-Seamless.cmd 启动并重新组队。密码、存档不变。

修正：战斗阶段切换不再依赖 UI 忙碌标记；击杀横幅改为单次本地触发；传送门独立激活。
粪金龟改用地图预置实体，双方加载完成后等待 3 秒，再开放 3 秒捕捉窗口。
付款至结算期间阻止传送门提前离开。保留每人最多 3 次和独立结算。
使用当前源码的队伍协议，所有队友必须同时更新 DLL。

已通过代码回归测试、资源回读和文件校验；双人实机仍待验证。
请优先验证：击杀后仅出现一次提示、触碰传送门后双方领奖、粪金龟三次游玩及结束后传送门。
如仍有问题，请保留双方 runtime/tower-preview.log。
''',encoding='utf-8-sig')
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
    for p in sorted(patch.rglob('*')):
        if p.is_file(): z.write(p,p.relative_to(patch))
with zipfile.ZipFile(archive) as z: assert z.testzip() is None
archive.with_suffix('.zip.sha256').write_text(f'{sha(archive)}  {archive.name}\n',encoding='ascii')
print(f'{archive}\n{archive.stat().st_size/1024:.1f} KiB; {len(manifest)} assets, native hash and ZIP CRC verified')
