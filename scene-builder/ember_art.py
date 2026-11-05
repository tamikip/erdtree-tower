"""余烬徽记：把生成的图集切成 160×160 图标。

用法：python scene-builder/ember_art.py scene-assets
读取 scene-assets/ember-art/emblems.png（4 列 × 3 行，Ember::ALL 前 12 项）、emblems-summon.png（2 列 × 2 行，召唤流 4 项）、emblems-twin.png（单个，双生的灵唤铃）、emblems-stack.png（2 列 × 2 行，叠层余烬 4 项）、emblems-mage.png（2 列 × 2 行，法师余烬 4 项）、emblems-footwork.png（单个，黑刀刺客的身法）、emblems-resonance.png（单个，灵唤师的共鸣）、emblems-pouch.png（单个，行商的无底囊）与 emblems-arts.png（单个，战技的精粹），
透明背景，按 src/embers.rs 中 Ember::ALL 的顺序从左到右、从上到下，
写出 scene-assets/reward-icons/990000.rgba … 990030.rgba（与原版图标相同的 160×160 RGBA 原始数据）。
游戏发现全部徽记都在时，奖励卡改用这些徽记；缺任何一张则继续借用原版护符图标。
"""
from pathlib import Path
import sys
from PIL import Image

FIRST, SIZE = 990000, 160
ATLASES = [("emblems.png", 4, 3, ["无声处刑", "鲜血君王的赐福", "永冻吐息", "死诞之焰", "米凯拉之刃", "荷莱·露的再起",
                                  "古龙之心", "以血还血", "癫火狂热", "圣杯余响", "噬神之蛇的饱食", "荆棘之躯"]),
           ("emblems-summon.png", 2, 2, ["仿身泪滴的替身", "灵骸迸裂", "众灵的盛宴", "灵唤铃的号令"]),
           ("emblems-twin.png", 1, 1, ["双生的灵唤铃"]),
           ("emblems-stack.png", 2, 2, ["噬魂者", "处刑之链", "屠神者", "灵魂收容"]),
           ("emblems-mage.png", 2, 2, ["托普斯的力场", "亚兹勒的冥想", "辉石的余韵", "蕾娜拉的满月"]),
           ("emblems-footwork.png", 1, 1, ["黑刀刺客的身法"]),
           ("emblems-resonance.png", 1, 1, ["灵唤师的共鸣"]),
           ("emblems-pouch.png", 1, 1, ["行商的无底囊"]),
           ("emblems-arts.png", 1, 1, ["战技的精粹"]),
           ("emblems-bulwark.png", 1, 1, ["坩埚骑士的铁壁"]),
           ("emblems-breaker.png", 1, 1, ["鲜血骑士的破势"])]

assets = Path(sys.argv[1])
destination = assets / "reward-icons"
first = FIRST
for file, COLUMNS, ROWS, NAMES in ATLASES:
    source = assets / "ember-art" / file
    with Image.open(source) as atlas:
        atlas = atlas.convert("RGBA")
        width, height = atlas.size
        tile_w, tile_h = width / COLUMNS, height / ROWS
        if abs(tile_w - tile_h) > max(tile_w, tile_h) * 0.05:
            sys.exit(f"{file} 应为 {COLUMNS}:{ROWS}（每格接近正方形），实际 {width}×{height}")
        if atlas.getextrema()[3][0] == 255:
            sys.exit(f"{file} 没有透明背景，请重新生成透明背景版本")
        for index, name in enumerate(NAMES):
            col, row = index % COLUMNS, index // COLUMNS
            box = (round(col * tile_w), round(row * tile_h), round((col + 1) * tile_w), round((row + 1) * tile_h))
            tile = atlas.crop(box)
            # 去掉四周的透明留白后居中放回正方形，保证每个徽记大小一致
            bounds = tile.getbbox()
            if bounds is None:
                sys.exit(f"{file} 第 {index + 1} 格（{name}）是空的")
            tile = tile.crop(bounds)
            side = max(tile.size)
            square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
            square.paste(tile, ((side - tile.width) // 2, (side - tile.height) // 2))
            icon = square.resize((SIZE - 12, SIZE - 12), Image.Resampling.LANCZOS)
            canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
            canvas.paste(icon, (6, 6))
            (destination / f"{first + index}.rgba").write_bytes(canvas.tobytes())
            canvas.save(assets / "ember-art" / f"{first + index}.png")
    print(f"Wrote {len(NAMES)} ember emblems ({first}-{first + len(NAMES) - 1}) from {file}")
    first += len(NAMES)
