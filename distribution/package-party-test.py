from pathlib import Path
import hashlib
import json
import shutil
import zipfile
import subprocess
import os
from native_build import build_native

project = Path(__file__).resolve().parents[1]
workspace = project.parents[1]
source = project / 'scene-assets'
name = 'ErdtreeTower-Multiplayer-Test-06'
target = project / 'dist' / name
archive = target.with_suffix('.zip')
seamless = Path('C:/Users/TamikiP/Downloads/Seamless Co-op v2.0.1 510 2.0.1 2026-09-02T23-30Z luvfLWq6V.zip')
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
with zipfile.ZipFile(seamless) as original:
    entries=[n for n in original.namelist() if n.lower().endswith('/ersc.dll')]
    assert len(entries)==1
    assert hashlib.sha256(original.read(entries[0])).hexdigest()=='fcd11a1849fd37851bb449be30f1525cf498f63fd1fa568c4e15030ebe7cb251'
    assert original.testzip() is None
if target.exists() or archive.exists():
    raise SystemExit('Package already exists; do not overwrite.')
built_native = build_native(project)
target.mkdir(parents=True)
shutil.copy2(built_native, target/'erdtree_tower_party_test.dll')
(target/'native.sha256').write_text(sha(target/'erdtree_tower_party_test.dll')+'\n',encoding='ascii')
for folder, relative in [(source,'scene-assets'),(project/'release-preview/loader/bin','loader/bin'),(project/'licenses','licenses')]:
    shutil.copytree(folder,target/relative)
assets = target/'scene-assets'
manifest = [{'Path':p.relative_to(assets).as_posix(),'SHA256':sha(p).upper()}
            for p in sorted(assets.rglob('*')) if p.is_file()]
(target/'assets.sha256.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8-sig')
for license in ['LICENSE-MIT','LICENSE-APACHE']:
    shutil.copy2(project/'release-preview/loader'/license,target/'loader'/license)
shutil.copy2(project/'THIRD_PARTY_NOTICES.md',target/'THIRD_PARTY_NOTICES.md')
(target/'dependencies').mkdir()
shutil.copy2(seamless,target/'dependencies/Seamless-Coop-2.0.1.zip')
launcher=(project/'Launch-PartyTest.ps1').read_text(encoding='utf-8-sig')
start=launcher.index("$native =")
launcher='''param([string]$GameDirectory, [switch]$CheckOnly)
$ErrorActionPreference = 'Stop'
$stage = $PSScriptRoot
$pathFile = Join-Path $stage 'GamePath.txt'
if (-not $GameDirectory -and (Test-Path -LiteralPath $pathFile)) { $GameDirectory = [IO.File]::ReadAllText($pathFile).Trim() }
if (-not $GameDirectory) {
    if ($CheckOnly) { throw 'Specify -GameDirectory for validation.' }
    $GameDirectory = Read-Host 'Paste the Game folder path containing eldenring.exe'
}
$GameDirectory = $GameDirectory.Trim().Trim('"')
$exe = Join-Path $GameDirectory 'eldenring.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Select the Game folder containing eldenring.exe.' }
$GameDirectory = (Resolve-Path -LiteralPath $GameDirectory).Path
if ((Get-FileHash -LiteralPath $exe).Hash -ne '1A3547101327F65D0C76DA2F9190AC0AA66871EA42BAE2AECC61E11A8B597891') { throw 'Game version mismatch. This test requires the inspected 1.17.1 build.' }
$loader = Join-Path $stage 'loader/bin/me3.exe'
''' + launcher[start:]
needle="if (Get-Process eldenring,ersc_launcher -ErrorAction SilentlyContinue) { throw 'Exit the running game before starting the isolated test.' }"
assert launcher.count(needle)==1
launcher=launcher.replace(needle,needle+"\n[IO.File]::WriteAllText($pathFile,$GameDirectory,[Text.UTF8Encoding]::new($true))")
(target/'Launch-Tower.ps1').write_text(launcher,encoding='utf-8-sig')
(target/'Start-Seamless.cmd').write_text('@echo off\nsetlocal\n"%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Launch-Tower.ps1" %*\nset "tower_result=%errorlevel%"\nif not "%tower_result%"=="0" pause\nexit /b %tower_result%\n',encoding='ascii')
guide='''黄金树爬塔 · 多人测试版 06

本包供双方共同测试，不代表完整联机通关已验证。
已做单人测试；第二、三幕改回竞技场的本次调整已通过构建和资源检查，尚待实机确认。

安装和启动
1. 双方完整解压本包到独立文件夹，例如 D:\\Mods\\ErdtreeTower-Test06。不要覆盖旧包，不要在 ZIP 内运行。
2. Steam 中找到艾尔登法环的 Game 文件夹，里面必须有 eldenring.exe。本包匹配已检查的游戏 1.17.1，启动时校验游戏文件。
3. 如果已安装无缝 2.0.1，可跳过此步。否则解压 dependencies\\Seamless-Coop-2.0.1.zip，将完整 SeamlessCoop 文件夹和 ersc_launcher.exe 放入上述 Game 文件夹；不要多套一层目录。已安装其他版本时先备份原文件和配置。
4. 双方编辑 Game\\SeamlessCoop\\ersc_settings.ini，将 cooppassword 设为相同的非空密码。不要发送自己的整个存档文件夹。
5. 打开 Steam、退出正在运行的游戏，双击本包 Start-Seamless.cmd，首次粘贴 Game 文件夹路径。以后会记住路径。
6. 用这个入口启动两台电脑。ME3 会一起加载无缝和爬塔。不要另开 ersc_launcher.exe；它不会自行加载本包。
7. 进入测试角色，坐一次赐福获得无缝道具，房主使用开放世界的道具，朋友使用加入世界的道具；双方角色进入同一房间后再开始爬塔。

Game 目录必要结构
Game\\eldenring.exe
Game\\SeamlessCoop\\ersc.dll
Game\\SeamlessCoop\\ersc_settings.ini
Game\\SeamlessCoop\\locale\\...

存档和版本
使用独立扩展名 ertowertest，与此前本机 stage-02 至 stage-06 测试角色一致。
不会自动读取旧正式包的 ertowercoop 或原版 sl2。请不要手动覆盖或改名迁移存档。
本包没有任何玩家存档、私人密码、GamePath.txt 或运行日志。
双方必须使用同次构建的完整 Test-06 包，不与 v94/v95 补丁或其他测试版混用。
本包 DLL 从当前源码重新构建，包含 DX12 后备缓冲区命令队列关联修复。
场景资源来自当前项目；商店固定供应四种抗性肝脏干和七种苔药，各 10 个。

本次场地
第一幕：宁姆格福竞技场；第二幕：盖利德竞技场；第三幕：王城竞技场。
各幕怪物池不变。营地服务 NPC 仅在营地节点开启。

优先验证
双方 NPC 可见与交互；所有人投票后的统一传送；敌人、击杀和 Boss 血条同步；各自领奖；营地互不抢次数；全队死亡后回圆桌重置。
事件守卫／粪金龟、断线重连和中途加入仍有未完成或未验证环节，暂不能保证完整多人流程。
出现问题时记录房主或客机、节点类型、具体步骤；日志位于本包 runtime\\tower-preview.log。
若提示缺少 ersc_settings.ini，请检查第3步的目录层级；若游戏目录已有 dinput8.dll，启动器会停止，请先排查原有加载器，不要随意覆盖。

本包包含用户提供的无缝 2.0.1 原始下载压缩包，未把本机运行配置拷入其中。
来源：https://www.nexusmods.com/eldenring/mods/510
'''
(target/'安装与联机测试.txt').write_text(guide,encoding='utf-8-sig')
with (target/'THIRD_PARTY_NOTICES.md').open('a',encoding='utf-8') as stream:
    stream.write('\nSeamless Co-op 2.0.1: the user-provided original download archive is included unchanged under dependencies. See its bundled notices. https://www.nexusmods.com/eldenring/mods/510\n')
subprocess.run([str(Path(os.environ['SystemRoot'])/'System32/WindowsPowerShell/v1.0/powershell.exe'),'-NoProfile','-ExecutionPolicy','Bypass','-File',str(target/'Launch-Tower.ps1'),'-GameDirectory','F:/SteamLibrary/steamapps/common/ELDEN RING/Game','-CheckOnly'],check=True,env={k:v for k,v in os.environ.items() if k.lower()!='psmodulepath'})
files=sorted(p for p in target.rglob('*') if p.is_file())
for p in files:
    assert not p.is_symlink() and not (p.stat().st_file_attributes & 0x400)
    assert p.suffix not in ['.log','.sl2','.co2','.ertowertest','.ertowercoop','.party-award']
    assert p.name not in ['GamePath.txt','host.json','guest.json','ersc_settings.ini']
(target/'SHA256SUMS.txt').write_text(''.join(f'{sha(p)}  {p.relative_to(target).as_posix()}\n' for p in files),encoding='utf-8')
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED,compresslevel=6) as output:
    for p in sorted(target.rglob('*')):
        if p.is_file(): output.write(p,Path(name)/p.relative_to(target))
with zipfile.ZipFile(archive) as output:
    assert output.testzip() is None
archive.with_suffix('.zip.sha256').write_text(f'{sha(archive)}  {archive.name}\n',encoding='ascii')
print(f'{archive}\n{archive.stat().st_size/1024**2:.1f} MiB; {len(files)+1} files; ZIP CRC verified')
