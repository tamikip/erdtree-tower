"""Minimal read-only Elden Ring regulation.bin reader (HansFix).
usage: python reg.py regulation.bin                      -> list params
       python reg.py regulation.bin ParamName id [id ...] -> row presence
       python reg.py regulation.bin ParamName id --hex     -> dump the row bytes"""
import struct, sys, zstandard, zlib
from Crypto.Cipher import AES
KEY = bytes.fromhex("99BFFC366A6BC8C6F5827D093602D676C42892A01C207FB024D3AF4E493FEF99")

def decrypt(path):
    raw = open(path, 'rb').read()
    data = AES.new(KEY, AES.MODE_CBC, raw[:16]).decrypt(raw[16:])
    assert data[:4] == b'DCX\0', data[:4]
    usize = struct.unpack('>I', data[0x1C:0x20])[0]
    fmt = data[0x28:0x2C]
    body = data[0x4C:]
    if fmt == b'ZSTD':
        return zstandard.ZstdDecompressor().decompress(body, max_output_size=usize)
    if fmt == b'DFLT':
        return zlib.decompress(body)
    raise SystemExit(f'unsupported DCX {fmt}')

def bnd4(data):
    assert data[:4] == b'BND4'
    count = struct.unpack_from('<i', data, 0x0C)[0]
    fhs = struct.unpack_from('<q', data, 0x20)[0]
    unicode = data[0x30]
    files = {}
    for i in range(count):
        o = 0x40 + i * fhs
        size = struct.unpack_from('<q', data, o + 8)[0]
        off = struct.unpack_from('<I', data, o + 0x18)[0]
        name_off = struct.unpack_from('<I', data, o + 0x20)[0]
        if unicode:
            end = name_off
            while data[end:end + 2] != b'\0\0': end += 2
            name = data[name_off:end].decode('utf-16-le')
        else:
            name = data[name_off:data.index(b'\0', name_off)].decode('shift_jis')
        files[name.replace('\\', '/').split('/')[-1].rsplit('.', 1)[0]] = data[off:off + size]
    return files

def rows(param):
    count = struct.unpack_from('<H', param, 0x0A)[0]
    return [struct.unpack_from('<iiq', param, 0x40 + 24 * i)[0::2] for i in range(count)]

if __name__ == '__main__':
    files = bnd4(decrypt(sys.argv[1]))
    if len(sys.argv) == 2:
        print('\n'.join(sorted(files))); raise SystemExit
    p = files[sys.argv[2]]
    table = rows(p)
    offsets = dict(table)
    size = table[1][1] - table[0][1] if len(table) > 1 else 0
    print(f'{sys.argv[2]}: {len(table)} rows, row size {size}')
    args = [a for a in sys.argv[3:] if a != '--hex']
    for a in args:
        rid = int(a)
        if rid not in offsets: print(rid, 'MISSING'); continue
        print(rid, 'present')
        if '--hex' in sys.argv:
            print(p[offsets[rid]:offsets[rid] + size].hex())
