"""Launcher artwork derived from the mod's own assets (run from the repo root: python launcher/make_art.py).

  ui/assets/img/tower-banner-<n>.webp  carousel banners, 768x356 (2x the 384x178 slot): one class each, the whole
                                       figure from scene-assets/class-art/classes.png on the right, name and role on the left
  ui/assets/img/tower-title.png        title logo overlay, 2560x1440 (2x the 1280x720 page, drawn above the carousel)
  ui/assets/img/tower-knight.webp      knight portrait on the right of the start page (transparent), from launcher/art/tower-knight.png
                                       (generated, prompt in launcher/knight-generation.txt), trimmed and scaled to 2x its 650 px slot

Class names, roles, signature mechanics and accent colours follow ROLES in native/src/class_ui.rs.
"""
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageEnhance, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'launcher/ui/assets/img'
TITLE_FONT = ROOT / 'scene-assets/fonts/TowerTitleSerif-Bold.otf'
BODY_FONT = ROOT / 'scene-assets/fonts/TowerSerif-Regular.otf'

# (name, role, signature mechanic, accent) in atlas order, row-major 3x2
ROLES = [
    ('守誓骑士', '坦克 · 举盾防守', '誓守', (196, 160, 96)),
    ('狂战士', '重武器 · 硬抗', '狂血', (200, 84, 64)),
    ('影刃', '敏捷 · 致命一击', '影步', (150, 172, 190)),
    ('星辉术士', '远程法术', '星涌', (110, 140, 220)),
    ('祷誓者', '祷告 · 治疗 · 辅助', '圣誓', (226, 196, 120)),
    ('召唤师', '骨灰为核心', '群灵', (140, 196, 170)),
]
GOLD_TOP, GOLD_BOTTOM = (250, 232, 178), (190, 138, 52)


def vertical_gradient(size, top, bottom):
    w, h = size
    column = Image.new('RGB', (1, h))
    for y in range(h):
        t = y / max(1, h - 1)
        column.putpixel((0, y), tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    return column.resize(size)


def ramp(size, start, end, horizontal=True):
    """Alpha ramp: 0 before start, 255 after end (pixels along the axis)."""
    w, h = size
    length = w if horizontal else h
    line = Image.new('L', (length, 1) if horizontal else (1, length))
    line.putdata([round(255 * min(1, max(0, (i - start) / (end - start)))) for i in range(length)])
    return line.resize((w, h))


def text_mask(size, xy, text, font, spacing=0):
    mask = Image.new('L', size)
    draw = ImageDraw.Draw(mask)
    if spacing:
        x, y = xy
        for ch in text:
            draw.text((x, y), ch, font=font, fill=255)
            x += font.getlength(ch) + spacing
    else:
        draw.text(xy, text, font=font, fill=255)
    return mask


def gilded(canvas, mask, height_box, shadow=6, glow=None, outline=1):
    """Gold-gradient lettering with a dark rim, soft drop shadow and an optional coloured glow."""
    size = canvas.size
    if glow:
        colour, radius, strength = glow
        halo = mask.filter(ImageFilter.GaussianBlur(radius)).point(lambda v: round(v * strength))
        canvas.alpha_composite(Image.merge('RGBA', (*Image.new('RGB', size, colour).split(), halo)))
    if shadow:
        dark = ImageChops.offset(mask, 0, shadow // 2).filter(ImageFilter.GaussianBlur(shadow)).point(lambda v: round(v * 0.85))
        canvas.alpha_composite(Image.merge('RGBA', (*Image.new('RGB', size, (8, 6, 4)).split(), dark)))
    if outline:
        rim = mask.filter(ImageFilter.MaxFilter(outline * 2 + 1))
        canvas.alpha_composite(Image.merge('RGBA', (*Image.new('RGB', size, (40, 26, 8)).split(), rim)))
    top, bottom = height_box
    fill = Image.new('RGB', size, GOLD_BOTTOM)
    fill.paste(vertical_gradient((size[0], bottom - top), GOLD_TOP, GOLD_BOTTOM), (0, top))
    canvas.alpha_composite(Image.merge('RGBA', (*fill.split(), mask)))


def banner(atlas, index):
    name, role, mechanic, accent = ROLES[index]
    W, H = 768, 356
    tile = atlas.crop(((index % 3) * 512, (index // 3) * 512, (index % 3) * 512 + 512, (index // 3) * 512 + 512))
    # Backdrop: the same tile blown up, blurred and darkened, so the figure's own background continues to the left edge
    back = tile.resize((W, W)).crop((0, 120, W, 120 + H)).filter(ImageFilter.GaussianBlur(22))
    back = ImageEnhance.Brightness(back).enhance(0.42).convert('RGBA')
    tint = Image.new('RGBA', (W, H), accent + (0,))
    tint.putalpha(ramp((W, H), W, 0).point(lambda v: round(v * 0.16)))
    back.alpha_composite(tint)
    # Figure: head to knees (the tile's lower fifth already fades to black), height-fitted on the right
    figure = tile.crop((0, 0, 512, 446)).resize((round(512 * H / 446), H), Image.LANCZOS).convert('RGBA')
    fw = figure.width
    figure.putalpha(ImageChops.multiply(ramp((fw, H), 0, 150), ramp((fw, H), H - 70, H - 10, horizontal=False).point(lambda v: 255 - round(v * 0.55))))
    back.alpha_composite(figure, (W - fw + 6, 0))
    # Left shade for the text column
    shade = Image.new('RGBA', (W, H), (6, 5, 4, 0))
    shade.putalpha(ramp((W, H), 430, 120).point(lambda v: round(v * 0.62)))
    back.alpha_composite(shade)

    title = ImageFont.truetype(str(TITLE_FONT), 62)
    body = ImageFont.truetype(str(BODY_FONT), 26)
    small = ImageFont.truetype(str(BODY_FONT), 22)
    x, y = 46, 104
    gilded(back, text_mask((W, H), (x, y), name, title, spacing=4), (y, y + 76), shadow=6, glow=(accent, 14, 0.55))
    draw = ImageDraw.Draw(back)
    rule_y = y + 96
    draw.line((x, rule_y, x + 150, rule_y), fill=accent + (210,), width=2)
    draw.polygon([(x + 162, rule_y - 5), (x + 167, rule_y), (x + 162, rule_y + 5), (x + 157, rule_y)], fill=accent + (230,))
    draw.text((x, rule_y + 18), role, font=body, fill=(226, 218, 200, 255))
    draw.text((x, rule_y + 60), f'专属机制 · {mechanic}', font=small, fill=tuple(min(255, round(c * 1.15)) for c in accent) + (255,))
    # Rounded slot corners are applied by the page; keep the image opaque
    return back.convert('RGB')


def title_overlay():
    W, H = 2560, 1440
    canvas = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    font = ImageFont.truetype(str(TITLE_FONT), 150)
    latin = ImageFont.truetype(str(TITLE_FONT), 38)
    x, y = 256, 418  # page coordinates (128, 209), left-aligned with the carousel below
    gilded(canvas, text_mask((W, H), (x, y), '黄金树爬塔', font, spacing=14), (y + 10, y + 176), shadow=14,
           glow=((214, 160, 64), 30, 0.45), outline=2)
    rule_y = y + 222
    draw = ImageDraw.Draw(canvas)
    draw.line((x + 4, rule_y, x + 120, rule_y), fill=(214, 172, 90, 220), width=3)
    draw.polygon([(x + 136, rule_y - 8), (x + 144, rule_y), (x + 136, rule_y + 8), (x + 128, rule_y)], fill=(232, 196, 120, 240))
    gilded(canvas, text_mask((W, H), (x + 160, rule_y - 24), 'ERDTREE TOWER', latin, spacing=10), (rule_y - 20, rule_y + 24),
           shadow=6, outline=1)
    return canvas


def knight():
    art = Image.open(ROOT / 'launcher/art/tower-knight.png').convert('RGBA')
    art = art.crop(art.getchannel('A').point(lambda v: 255 if v > 8 else 0).getbbox())
    height = 1300
    return art.resize((round(art.width * height / art.height), height), Image.LANCZOS)


if __name__ == '__main__':
    atlas = Image.open(ROOT / 'scene-assets/class-art/classes.png').convert('RGB')
    for i in range(len(ROLES)):
        banner(atlas, i).save(OUT / f'tower-banner-{i + 1}.webp', quality=90, method=6)
    title_overlay().save(OUT / 'tower-title.png', optimize=True)
    knight().save(OUT / 'tower-knight.webp', quality=88, method=6)
    print('launcher art written to', OUT)
