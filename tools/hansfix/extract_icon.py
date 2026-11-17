"""Extract a native menu icon (by layout id substring) from 01_common.tpf.dcx and save a tinted 160x160 RGBA.
usage: python extract_icon.py <game dir> <scene-assets dir> <icon id> <out.rgba> [--list] [--png out.png]"""
import ctypes, struct, sys, re, pathlib

def oodle(game):
    lib = ctypes.WinDLL(str(pathlib.Path(game) / 'oo2core_6_win64.dll'))
    fn = lib.OodleLZ_Decompress
    fn.restype = ctypes.c_int64
    fn.argtypes = [ctypes.c_char_p, ctypes.c_int64, ctypes.c_char_p, ctypes.c_int64] + [ctypes.c_int] * 3 + [ctypes.c_void_p, ctypes.c_int64, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64, ctypes.c_int]
    return fn

def dcx(path, dec):
    d = open(path, 'rb').read()
    assert d[:4] == b'DCX\0' and d[0x28:0x2C] == b'KRAK', d[0x28:0x2C]
    usize, csize = struct.unpack('>II', d[0x1C:0x24])
    out = ctypes.create_string_buffer(usize)
    n = dec(d[0x4C:0x4C + csize], csize, out, usize, 0, 0, 0, None, 0, None, None, None, 0, 3)
    assert n == usize, (n, usize)
    return out.raw

def bnd4(data):
    n = struct.unpack_from('<i', data, 0xC)[0]; fhs = struct.unpack_from('<q', data, 0x20)[0]; uni = data[0x30]
    files = {}
    for i in range(n):
        o = 0x40 + i * fhs
        size = struct.unpack_from('<q', data, o + 8)[0]; off = struct.unpack_from('<I', data, o + 0x18)[0]; no = struct.unpack_from('<I', data, o + 0x20)[0]
        e = no
        if uni:
            while data[e:e + 2] != b'\0\0': e += 2
            name = data[no:e].decode('utf-16-le')
        else:
            name = data[no:data.index(b'\0', no)].decode('shift_jis')
        files[name] = data[off:off + size]
    return files

def tpf(data):
    assert data[:4] == b'TPF\0'
    count = struct.unpack_from('<i', data, 8)[0]
    enc = data[0xE]
    out = {}
    for i in range(count):
        o = 0x10 + i * 0x14
        off, size, fmt, typ, mips, flags, noff = struct.unpack_from('<IIBBBBI', data, o)
        if enc == 1:
            e = noff
            while data[e:e + 2] != b'\0\0': e += 2
            name = data[noff:e].decode('utf-16-le')
        else:
            name = data[noff:data.index(b'\0', noff)].decode('shift_jis')
        out[name] = data[off:off + size]
    return out

def main():
    game, assets, icon, outp = sys.argv[1:5]
    dec = oodle(game)
    layout = bnd4(dcx(pathlib.Path(assets) / 'menu/hi/01_common.sblytbnd.dcx', dec))
    hits = []
    for name, data in layout.items():
        text = data.decode('utf-8', 'replace')
        image = re.search(r'imagePath="([^"]+)"', text)
        for m in re.finditer(r'<SubTexture[^>]*>', text):
            tag = m.group(0)
            n = re.search(r'name="([^"]+)"', tag).group(1)
            if re.search(r'(?<!\d)' + icon + r'(?!\d)', n):
                v = {k: int(re.search(k + r'="(\d+)"', tag).group(1)) for k in ('x', 'y', 'width', 'height')}
                hits.append((n, image.group(1) if image else '?', v))
    for h in hits: print('match', h)
    if '--list' in sys.argv or not hits: return
    name, imagepath, r = hits[0]
    atlas = pathlib.Path(imagepath.replace('\\', '/')).stem
    textures = tpf(dcx(pathlib.Path(assets) / 'menu/hi/01_common.tpf.dcx', dec))
    dds = textures[atlas]
    h, w = struct.unpack_from('<II', dds, 12)
    fourcc = dds[84:88]
    print('atlas', atlas, w, h, fourcc, struct.unpack_from('<I', dds, 128)[0] if fourcc == b'DX10' else '')
    import texture2ddecoder
    hdr = 148 if fourcc == b'DX10' else 128
    bgra = texture2ddecoder.decode_bc7(dds[hdr:hdr + w * h], w, h)
    from PIL import Image
    img = Image.frombytes('RGBA', (w, h), bgra, 'raw', 'BGRA').crop((r['x'], r['y'], r['x'] + r['width'], r['y'] + r['height'])).resize((160, 160), Image.LANCZOS)
    px = img.load()
    for yy in range(160):
        for xx in range(160):
            R, G, B, A = px[xx, yy]
            lum = 0.3 * R + 0.59 * G + 0.11 * B
            # 浅蓝色滤镜：亮度提高 25% 后乘上浅蓝 (120,205,255)，与原色按 80% 混合
            tr, tg, tb = min(255, lum * 1.25 * 120 / 255 + 10), min(255, lum * 1.25 * 205 / 255 + 25), min(255, lum * 1.25 + 40)
            px[xx, yy] = (int(R * 0.2 + tr * 0.8), int(G * 0.2 + tg * 0.8), int(min(255, B * 0.2 + tb * 0.8)), A)
    open(outp, 'wb').write(img.tobytes())
    if '--png' in sys.argv: img.save(sys.argv[sys.argv.index('--png') + 1])
    print('wrote', outp, len(img.tobytes()))

main()
