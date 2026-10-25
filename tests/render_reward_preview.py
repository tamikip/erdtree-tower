from pathlib import Path
import sys
import numpy as np
from PIL import Image

root, assets = map(Path, sys.argv[1:])
width, height = map(int, (root/'font.size').read_text().split())
textures = {0: np.frombuffer((root/'font.rgba').read_bytes(), dtype=np.uint8).reshape(height, width, 4)}
for texture, name, w, h in [(100000,'paper',1024,1536),(100001,'icons',1254,1254)]:
    path=assets/'map-art'/f'{name}.rgba'
    if path.exists(): textures[texture]=np.frombuffer(path.read_bytes(),dtype=np.uint8).reshape(h,w,4)
for file in (assets/'reward-icons').glob('*.rgba'):
    textures[int(file.stem)+1] = np.frombuffer(file.read_bytes(), dtype=np.uint8).reshape(160, 160, 4)
for index,(name,w,h) in enumerate([('cards',1254,1254),('background',1672,941),('button',2172,724)]):
    path=assets/'oath-art'/f'{name}.rgba'
    if path.exists(): textures[200000+index]=np.frombuffer(path.read_bytes(),dtype=np.uint8).reshape(h,w,4)
textures[200003]=np.frombuffer((assets/'reward-art/card.rgba').read_bytes(),dtype=np.uint8).reshape(1536,1024,4)
textures[200004]=np.frombuffer((assets/'reward-art/rarity-cards.rgba').read_bytes(),dtype=np.uint8).reshape(1024,1536,4)
textures[200005]=np.frombuffer((assets/'reward-art/rare-blue.rgba').read_bytes(),dtype=np.uint8).reshape(1774,887,4)
for file in root.glob('*.draw'):
    with file.open() as stream:
        w, h = map(int, next(stream).split())
        canvas = np.full((h, w, 4), (45, 48, 43, 255), dtype=np.float32)
        for line in stream:
            p = line.split()
            if p[0] == 'C':
                texture = textures[int(p[1])]
                clip = list(map(float, p[2:]))
                continue
            v = np.array(list(map(float, p[1:])), dtype=np.float32).reshape(3, 8)
            x0, y0 = max(0, int(np.floor(v[:,0].min())), int(clip[0])), max(0, int(np.floor(v[:,1].min())), int(clip[1]))
            x1, y1 = min(w, int(np.ceil(v[:,0].max())), int(clip[2])), min(h, int(np.ceil(v[:,1].max())), int(clip[3]))
            if x1 <= x0 or y1 <= y0: continue
            a, b, c = v[:, :2]
            det = (b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
            if abs(det)<1e-8: continue
            yy, xx = np.mgrid[y0:y1, x0:x1].astype(np.float32)
            xx += .5; yy += .5
            wa = ((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/det
            wb = ((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/det
            wc = 1-wa-wb
            mask = (wa>=0)&(wb>=0)&(wc>=0)
            if not mask.any(): continue
            weights = np.stack((wa[mask],wb[mask],wc[mask]),axis=1)
            values = weights @ v[:,2:]
            th, tw = texture.shape[:2]
            tx = np.clip((values[:,0]*tw).astype(int),0,tw-1)
            ty = np.clip((values[:,1]*th).astype(int),0,th-1)
            color = texture[ty,tx].astype(np.float32)*values[:,2:]/255
            alpha = color[:,3:4]/255
            dest = canvas[y0:y1,x0:x1]
            dest[mask,:3] = color[:,:3]*alpha+dest[mask,:3]*(1-alpha)
    Image.fromarray(canvas.astype(np.uint8)).save(file.with_suffix('.png'))
    print(file.with_suffix('.png'))
