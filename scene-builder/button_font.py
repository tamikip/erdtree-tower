"""结算页黑体：从思源黑体粗体（Noto Sans CJK SC Bold）裁出结算页用到的字，写出 scene-assets/fonts/TowerSans-Bold.otf
与字表 ui-sans-glyphs.txt（游戏据此判断一段文字能不能用黑体画，缺字的退回宋体）。

用法：python scene-builder/button_font.py <NotoSansCJK-Bold.ttc> <仓库根目录>
需要 fontTools。保留 ASCII、LABELS 与 SOURCES 里出现的字（结算页的文字、职业、余烬、誓约、律法、敌人与护符名，约一千字）。
游戏里按两个字号加载：正文 26 像素；大号数字与评级字母（DISPLAY）按 220 像素只加载这几个字。
字体按 SIL Open Font License 1.1 授权；裁剪属于修改，按许可改用新的字体名“Tower Sans”，许可全文见 licenses/NotoSansCJK-OFL.txt。
"""
from pathlib import Path
import sys
from fontTools.ttLib import TTCollection
from fontTools.subset import Options, Subsetter

LABELS = "返回大赐福档案复制种子再次挑战开始新旅程确定取消本局评分苦难律法总分新纪录历史最高登顶成功旅程终结暂离结算"
SOURCES = ["native/src/run_screen.rs", "native/src/law_ui.rs", "native/src/map_ui.rs", "src/embers.rs", "src/contracts.rs", "src/law.rs", "src/score.rs",
           "src/journal.rs", "src/rewards.rs", "scene-assets/boss-pool.txt", "scene-assets/enemy-pools.txt"]
DISPLAY = "0123456789SABCD+×."
source, root = Path(sys.argv[1]), Path(sys.argv[2])
chars = set(chr(c) for c in range(0x20, 0x7F)) | set(LABELS) | set(DISPLAY)
for name in SOURCES:
    chars |= {c for c in (root / name).read_text(encoding="utf-8", errors="ignore") if c.isprintable() or c == "\u3000"}
chars -= set("\r\n\t")
collection = TTCollection(str(source))
font = next(f for f in collection.fonts if f["name"].getDebugName(4) == "Noto Sans CJK SC Bold")
options = Options()
options.layout_features = ["*"]
options.name_IDs = ["*"]
options.notdef_outline = True
subsetter = Subsetter(options)
subsetter.populate(unicodes=[ord(c) for c in chars])
subsetter.subset(font)
# 改名：OFL 要求修改后的版本不使用原字体名
for record in font["name"].names:
    if record.nameID in (1, 16):
        record.string = "Tower Sans"
    elif record.nameID == 4:
        record.string = "Tower Sans Bold"
    elif record.nameID == 6:
        record.string = "TowerSans-Bold"
    elif record.nameID == 3:
        record.string = "Tower Sans Bold; subset of Noto Sans CJK SC Bold"
out = root / "scene-assets" / "fonts" / "TowerSans-Bold.otf"
font.save(str(out))
covered = sorted(c for c in chars if ord(c) in font.getBestCmap())
(root / "scene-assets" / "fonts" / "ui-sans-glyphs.txt").write_text("".join(c for c in covered if ord(c) >= 0x80), encoding="utf-8")
print(f"Wrote {out.name}: {len(covered)} of {len(chars)} characters, {out.stat().st_size / 1024:.0f} KB")
missing = [c for c in DISPLAY + LABELS if ord(c) not in font.getBestCmap()]
if missing:
    sys.exit(f"missing: {''.join(missing)}")
