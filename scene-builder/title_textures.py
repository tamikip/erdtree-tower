"""Use Elden Ring's external GFx images; DDS payloads are packed by title-texture-patch."""
import argparse
import io
import json
import struct
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

import gfx_bitmap as gb

ART = ('title', 'loading1', 'loading2', 'loading3')
MOVIES = {'05_000_title.gfx': ['title'], '02_903_nowloading2.gfx': list(ART[1:]),
          '02_904_nowloading3.gfx': list(ART[1:])}


def translated_matrix(data, x, y):
    pos = 0
    def read(count):
        nonlocal pos
        value = 0
        for _ in range(count):
            value = value << 1 | (data[pos // 8] >> (7-pos % 8) & 1)
            pos += 1
        return value
    for _ in range(2):
        if read(1):
            count = read(5)
            read(count * 2)
    prefix = pos
    count = read(5)
    read(count * 2)
    end = (pos + 7) // 8
    bits = gb.Bits()
    for i in range(prefix):
        bits.unsigned(data[i // 8] >> (7-i % 8) & 1, 1)
    count = gb.signed_bits(x, y)
    bits.unsigned(count, 5); bits.signed(x, count); bits.signed(y, count)
    return bits.data(), end


def title_underlay(data):
    head, tags = gb.parse(data)
    target = gb.symbols(tags)['_05_000_Title_fla.Timeline_1']
    index = next(i for i,(code,body) in enumerate(tags) if code==39 and struct.unpack_from('<H',body)[0]==target)
    body = tags[index][1]; inner = list(gb.read_tags(body,4,len(body)))
    art = next(b for c,b in inner if c==26 and b.endswith(b'TowerThemeArt\0'))
    if struct.unpack_from('<H',art,1)[0]==1:
        return data
    controls = {struct.unpack_from('<H',b,1)[0]:xy for c,b in inner if c==26
                for name,xy in [(b'CommandList\0',(-550,90)),(b'PressStart\0',(-550,160))] if b.endswith(name)}
    fixed = []
    for code,part in inner:
        if code in (4,5,70) or code==26 and part[0]&0x40:
            raise ValueError('Unsupported title placement or mask; do not silently alter it')
        if code==26:
            depth = struct.unpack_from('<H',part,1)[0]
            new_depth = 1 if part==art else depth+1
            if new_depth > 65535:
                raise ValueError('Title depth overflow')
            if depth in controls and part[0]&4:
                start = 3 + (2 if part[0]&2 else 0)
                matrix, used = translated_matrix(part[start:],*(v*20 for v in controls[depth]))
                part = part[:start] + matrix + part[start+used:]
            part = part[:1]+struct.pack('<H',new_depth)+part[3:]
        elif code==28:
            part = struct.pack('<H',struct.unpack_from('<H',part)[0]+1)
        fixed.append((code,part))
    tags[index] = (39,body[:4]+b''.join(gb.tag(c,b) for c,b in fixed))
    return gb.build(head,tags)


def title_art(image, config=None):
    root = Path(__file__).resolve().parents[1]
    config = config or json.loads(Path(__file__).with_name('title_theme.json').read_text(encoding='utf-8'))
    layer = Image.new('RGBA',image.size)
    draw = ImageDraw.Draw(layer)
    draw.rounded_rectangle((96,120,716,1000),9,fill=(11,15,14,158))
    gold = '#'+config['palette']['gold']
    draw.line([(130,348),(620,348)],fill=gold,width=2)
    for at,text,size,color,bold in [((128,170),config['subtitle'],28,gold,False),
        ((122,225),config['title'],config.get('title_size',82),'#'+config['palette']['paper'],True),
        ((130,375),config['tagline'],24,'#c9d2be',False),
        ((130,930),config.get('footer','每一次攀登，都是新的选择'),20,'#c9d2be',False)]:
        font=root/'scene-assets/fonts'/('TowerTitleSerif-Bold.otf' if bold else 'TowerSerif-Regular.otf')
        draw.text(at,text,font=ImageFont.truetype(str(font),size),fill=color)
    return Image.alpha_composite(image,layer)


def loading_underlay(data):
    head,tags=gb.parse(data)
    if any(c==26 and b.endswith(b'TowerThemeArtCycle\0') for c,b in tags):
        return data
    art=[b for c,b in tags if c==26 and any(b.endswith(('TowerThemeArt'+str(i)).encode()+b'\0') for i in range(3))]
    if len(art)!=3:
        raise ValueError('Expected three loading backgrounds')
    back=next((struct.unpack_from('<H',b,1)[0] for c,b in tags if c==26 and b.endswith(b'BackImage\0')),None)
    character=max(gb.max_character(tags),*(struct.unpack_from('<H',b,3)[0] for b in art))+1
    if character>65535:
        raise ValueError('Loading character ID overflow')
    cycle=[]
    for i,part in enumerate(art):
        if i:
            cycle.append((28,struct.pack('<H',1)))
        cycle.append((26,part[:1]+struct.pack('<H',1)+part[3:]))
        cycle.extend([(1,b'')]*240)
    cycle.append((0,b''))
    sprite=struct.pack('<HH',character,720)+b''.join(gb.tag(c,b) for c,b in cycle)
    out=[];added=False
    for code,part in tags:
        if code in (4,5,70) or code==26 and part[0]&0x40:
            raise ValueError('Unsupported loading placement or mask')
        if code==26:
            if part in art:
                if not added:
                    out.extend([(39,sprite),(26,gb.place(3 if back is not None else 2,character,'TowerThemeArtCycle'))])
                    added=True
                continue
            depth=struct.unpack_from('<H',part,1)[0]
            depth=1 if depth==1 else 2 if depth==back else depth+1
            part=part[:1]+struct.pack('<H',depth)+part[3:]
        elif code==28:
            depth=struct.unpack_from('<H',part)[0]
            part=struct.pack('<H',1 if depth==1 else 2 if depth==back else depth+1)
        elif code==39:
            inner=list(gb.read_tags(part,4,len(part)))
            knowledge={struct.unpack_from('<H',b,1)[0] for c,b in inner if c==26 and b.endswith(b'Knowledge\0')}
            if knowledge:
                fixed=[]
                for c,b in inner:
                    if c==26 and struct.unpack_from('<H',b,1)[0] in knowledge and b[0]&4:
                        start=3+(2 if b[0]&2 else 0)
                        matrix,used=translated_matrix(b[start:],-200000,0)
                        b=b[:start]+matrix+b[start+used:]
                    fixed.append((c,b))
                part=part[:4]+b''.join(gb.tag(c,b) for c,b in fixed)
        out.append((code,part))
    return gb.build(head,out)


def loading_art(image, index, config=None):
    root=Path(__file__).resolve().parents[1]
    config=config or json.loads(Path(__file__).with_name('title_theme.json').read_text(encoding='utf-8'))
    layer=Image.new('RGBA',image.size);draw=ImageDraw.Draw(layer)
    draw.rectangle((0,760,1920,1080),fill=(7,9,8,199))
    gold='#'+config['palette']['gold']
    draw.line([(120,762),(1800,762)],fill=gold,width=2)
    lines=[((130,785),config['subtitle']+'  ·  '+config['loading_title'],24,gold)]
    for at,text,size,color in lines:
        font=ImageFont.truetype(str(root/'scene-assets/fonts/TowerSerif-Regular.otf'),size)
        if draw.textlength(text,font=font)>790:
            raise ValueError('Loading tip exceeds its column: '+text)
        draw.text(at,text,font=font,fill=color)
    return Image.alpha_composite(image,layer)


def tip_atlases(output, config=None):
    root=Path(__file__).resolve().parents[1]
    tips=(config or json.loads(Path(__file__).with_name('title_theme.json').read_text(encoding='utf-8')))['tips']
    if len(tips)!=12:
        raise ValueError('Expected twelve loading tips')
    for page in range(2):
        atlas=Image.new('RGBA',(1920,1080));draw=ImageDraw.Draw(atlas)
        for slot,(title,body) in enumerate(tips[page*6:page*6+6]):
            for text,size,y,color in [(title,34,15,'#f1e9d5'),(body,25,78,'#d2d6c8')]:
                font=ImageFont.truetype(str(root/'scene-assets/fonts/TowerSerif-Regular.otf'),size)
                if draw.textlength(text,font=font)>1600:raise ValueError('Tip too wide')
                draw.text((155,slot*180+y),text,font=font,fill=color)
        atlas.save(output/f'tips{page+1}.dds')


def single_tip_cycle(data):
    head,tags=gb.parse(data)
    placed=next(b for c,b in tags if c==26 and b.endswith(b'TowerThemeArtCycle\0'))
    cycle_id=struct.unpack_from('<H',placed,3)[0]
    sprite=next(b for c,b in tags if c==39 and struct.unpack_from('<H',b)[0]==cycle_id)
    backgrounds=[b for c,b in gb.read_tags(sprite,4,len(sprite)) if c==26 and struct.unpack_from('<H',b,1)[0]==1][:3]
    if len(backgrounds)!=3:raise ValueError('Missing loading backgrounds')
    first=gb.max_character(tags)+1
    definitions=[];frames=[]
    for i in range(12):
        bitmap=first+i*2;shape=bitmap+1
        definitions.extend([(1009,external_image(bitmap,f'MENU_TowerTheme_tip{i}',(1920,180))),
                            (2,gb.bitmap_shape(shape,bitmap,1920,180,(1920,180)))])
        if i:frames.extend([(28,struct.pack('<H',1)),(28,struct.pack('<H',2))])
        frames.extend([(26,backgrounds[i%3]),(26,gb.place(2,shape,'TowerLoadingTip',0,825))])
        frames.extend([(1,b'')]*240)
    frames.append((0,b''))
    replacement=struct.pack('<HH',cycle_id,2880)+b''.join(gb.tag(c,b) for c,b in frames)
    out=[]
    for code,body in tags:
        if code==39 and struct.unpack_from('<H',body)[0]==cycle_id:
            out.extend(definitions);out.append((39,replacement))
        else:out.append((code,body))
    return gb.build(head,out)


def external_image(character, name, size):
    def string(value):
        raw = value.encode('ascii')
        if len(raw) >= 128:
            raise ValueError('GFx resource name too long')
        return bytes([len(raw)]) + raw
    return struct.pack('<IHHH', character, 13, *size) + string(name) + string(name + '.tga')


def externalize(data, names):
    head, tags = gb.parse(data)
    embedded = [(code, body) for code, body in tags if code == gb.DEFINE_BITS_JPEG2]
    if not embedded:
        present = [body for code, body in tags if code == 1009]
        if all(any(('MENU_TowerTheme_' + name + '.tga').encode() in body for body in present) for name in names):
            return data
        raise ValueError('Theme image definitions missing')
    if len(embedded) != len(names):
        raise ValueError('Unexpected image count; rebuild from original menus, not an already embedded theme')
    replacements = {}
    for (_, body), name in zip(embedded, names):
        with Image.open(io.BytesIO(body[2:])) as image:
            replacements[body] = external_image(struct.unpack_from('<H', body)[0], 'MENU_TowerTheme_' + name, image.size)
    return gb.build(head, [(1009, replacements[body]) if code == gb.DEFINE_BITS_JPEG2 else (code, body) for code, body in tags])


def write_dds(art, output, config=None):
    output.mkdir(parents=True, exist_ok=True)
    for name in ART:
        source = next((art / (name + suffix) for suffix in ('.png', '.jpg') if (art / (name + suffix)).exists()), None)
        if source is None:
            raise FileNotFoundError(name)
        with Image.open(source) as image:
            prepared = image.convert('RGBA').resize((1920, 1080), Image.Resampling.LANCZOS)
            if name=='title':
                prepared=title_art(prepared,config)
            else:
                prepared=loading_art(prepared,ART.index(name)-1,config)
            path = output / (name + '.dds')
            prepared.save(path)
            with Image.open(path) as restored:
                if restored.size != prepared.size or restored.convert('RGBA').tobytes() != prepared.tobytes():
                    raise ValueError('DDS pixel round-trip failed: ' + name)
    tip_atlases(output,config)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--art', type=Path, required=True)
    parser.add_argument('--textures-only', action='store_true', help='只重新生成 theme-textures/*.dds（菜单 GFx 已经引用外部贴图时用；主菜单现为原版）')
    args = parser.parse_args()
    if args.textures_only:
        write_dds(args.art, args.output / 'theme-textures')
        print('Wrote theme-textures; run title-texture-patch with the same output directory')
        raise SystemExit
    if args.source is None:
        parser.error('--source is required unless --textures-only')
    (args.output / 'menu').mkdir(parents=True, exist_ok=True)
    for filename, names in MOVIES.items():
        data=externalize((args.source / filename).read_bytes(), names)
        if filename=='05_000_title.gfx':
            data=title_underlay(data)
        else:
            data=single_tip_cycle(loading_underlay(data))
        (args.output / 'menu' / filename).write_bytes(data)
    filename = '05_001_title_logo.gfx'
    (args.output / 'menu' / filename).write_bytes((args.source / filename).read_bytes())
    write_dds(args.art, args.output / 'theme-textures')
    print('Converted theme images to external references; run title-texture-patch to pack DDS and layouts')
