"""词条护符的重绘图标：把生成的图集切成 160×160 图标。

用法：python scene-builder/talisman_art.py scene-assets
读取 scene-assets/talisman-art/talismans-1.png … talismans-9.png（各 4 列 × 3 行）与 talismans-10.png（单个），透明背景，
按 talisman-definitions.tsv 里各类护符第一次出现的顺序从左到右、从上到下排（共 109 类，四个稀有度共用一张），
写出 scene-assets/reward-icons/65100.rgba … 65208.rgba（与原版图标相同的 160×160 RGBA 原始数据），
并把 talisman-details.tsv 的图标列改成对应编号（奖励卡用）。游戏内的图标还要跑 talisman-icon-patch 与 talisman-icon-menu。
"""
from pathlib import Path
import sys
from PIL import Image

FIRST, SIZE = 65100, 160
ATLASES = [(f"talismans-{n}.png", 4, 3) for n in range(1, 10)] + [("talismans-10.png", 1, 1)]

assets = Path(sys.argv[1])
definitions = [line.split("\t") for line in (assets / "talisman-definitions.tsv").read_text(encoding="utf-8").splitlines()]
kinds = list(dict.fromkeys(row[4] for row in definitions))
if len(kinds) != sum(columns * rows for _, columns, rows in ATLASES):
    sys.exit(f"护符类数 {len(kinds)} 与图集格数不符")
index = 0
for file, columns, rows in ATLASES:
    with Image.open(assets / "talisman-art" / file) as atlas:
        atlas = atlas.convert("RGBA")
        width, height = atlas.size
        tile_w, tile_h = width / columns, height / rows
        if abs(tile_w - tile_h) > max(tile_w, tile_h) * 0.05:
            sys.exit(f"{file} 应为 {columns}:{rows}（每格接近正方形），实际 {width}×{height}")
        if atlas.getextrema()[3][0] == 255:
            sys.exit(f"{file} 没有透明背景，请重新生成透明背景版本")
        for cell in range(columns * rows):
            col, row = cell % columns, cell // columns
            tile = atlas.crop((round(col * tile_w), round(row * tile_h), round((col + 1) * tile_w), round((row + 1) * tile_h)))
            # 去掉四周的透明留白后居中放回正方形，保证每个护符大小一致（几乎透明的杂点不算）
            bounds = tile.getchannel("A").point(lambda a: 255 if a > 16 else 0).getbbox()
            if bounds is None:
                sys.exit(f"{file} 第 {cell + 1} 格（{kinds[index]}）是空的")
            tile = tile.crop(bounds)
            side = max(tile.size)
            square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
            square.paste(tile, ((side - tile.width) // 2, (side - tile.height) // 2))
            icon = square.resize((SIZE - 12, SIZE - 12), Image.Resampling.LANCZOS)
            canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
            canvas.paste(icon, (6, 6))
            (assets / "reward-icons" / f"{FIRST + index}.rgba").write_bytes(canvas.tobytes())
            index += 1
    print(f"{file}: talismans up to {FIRST + index - 1}")
icons = {kind: FIRST + i for i, kind in enumerate(kinds)}
details = (assets / "talisman-details.tsv").read_text(encoding="utf-8").splitlines()
if len(details) != len(definitions):
    sys.exit("talisman-details.tsv 与 talisman-definitions.tsv 行数不符")
lines = []
for line, definition in zip(details, definitions):
    fields = line.split("\t")
    if int(fields[0]) != 0x20000000 | int(definition[0]):
        sys.exit(f"talisman-details.tsv 顺序与定义不符：{fields[0]}")
    fields[1] = str(icons[definition[4]])
    lines.append("\t".join(fields))
(assets / "talisman-details.tsv").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(f"Wrote {len(kinds)} talisman icons ({FIRST}-{FIRST + len(kinds) - 1}) and updated talisman-details.tsv")
