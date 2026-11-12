"""Start each loading fade-in on a random art/tip segment without changing native timelines."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from native_name_glow import verify_tags

TARGETS = [('02_903_nowloading2', '_02_903_NowLoading2_fla'),
           ('02_904_nowloading3', '_02_904_NowLoading3_fla')]
MEMBERS = '''
      private var towerLoadingPending:Boolean = true;
      private var towerLoadingPrevious:int = -1;
      private function towerBeginLoading():void {
         towerLoadingPending = true;
      }
      private function towerChooseLoading(event:flash.events.Event):void {
         if(!towerLoadingPending || currentFrame == 1) return;
         var cycle:flash.display.MovieClip = getChildByName("TowerThemeArtCycle") as flash.display.MovieClip;
         if(cycle == null) return;
         var background:int = int(Math.random() * 3);
         if(towerLoadingPrevious >= 0) background = (towerLoadingPrevious + 1 + int(Math.random() * 2)) % 3;
         var tip:int = background + int(Math.random() * 4) * 3;
         if(cycle.totalFrames != 2880) return;
         cycle.gotoAndPlay(tip * 240 + 1);
         towerLoadingPrevious = background;
         towerLoadingPending = false;
      }
'''


def patch_source(source):
    anchor = '      public function MainTimeline()'
    frames = 'addFrameScript(88,this.frame89,119,this.frame120);'
    if source.count(anchor) != 1 or source.count(frames) != 1 or 'towerChooseLoading' in source:
        raise ValueError('Unexpected or already patched loading script')
    source = source.replace(anchor, MEMBERS + '\n' + anchor, 1)
    return source.replace(frames, 'addFrameScript(1,this.towerBeginLoading,88,this.frame89,119,this.frame120);\n'
                          '         addEventListener(flash.events.Event.ENTER_FRAME,this.towerChooseLoading);', 1)


def build(args):
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output/'menu').mkdir()
    records = []
    def run(*command):
        result = subprocess.run([args.java, '-Djava.awt.headless=true', '-jar', str(args.ffdec), *map(str, command)],
                                capture_output=True, text=True, encoding='utf-8', errors='replace')
        with (args.output/'build.log').open('a', encoding='utf-8') as log:
            log.write(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError('FFDec failed; see build.log')
    for stem, package in TARGETS:
        source = args.source/(stem+'.gfx'); target = args.output/'menu'/source.name
        with tempfile.TemporaryDirectory(prefix='tower-loading-') as folder:
            folder = Path(folder)
            run('-export', 'script', folder/'before', source)
            relative = Path(package)/'MainTimeline.as'
            script = (folder/'before/scripts'/relative).read_text(encoding='utf-8-sig')
            changed = folder/'changed'/relative; changed.parent.mkdir(parents=True)
            changed.write_text(patch_source(script), encoding='utf-8')
            run('-importScript', source, target, folder/'changed')
            count = verify_tags(source.read_bytes(), target.read_bytes())
            run('-export', 'script', folder/'after', target)
            for before in (folder/'before/scripts').rglob('*.as'):
                rel = before.relative_to(folder/'before/scripts')
                after = folder/'after/scripts'/rel
                if rel != relative and before.read_bytes() != after.read_bytes():
                    raise ValueError('Unrelated script changed: '+str(rel))
            actual = (folder/'after/scripts'/relative).read_text(encoding='utf-8-sig')
            for marker in ['towerChooseLoading', 'towerBeginLoading', 'gotoAndPlay', '2880', '240']:
                if marker not in actual: raise ValueError('Missing compiled rotation: '+marker)
            (args.output/(stem+'.as')).write_text(actual, encoding='utf-8')
        records.append(dict(file='menu/'+source.name, before=hashlib.sha256(source.read_bytes()).hexdigest(),
                            after=hashlib.sha256(target.read_bytes()).hexdigest(), tagsVerified=count))
    (args.output/'manifest.json').write_text(json.dumps(dict(gameVerified=False,files=records),indent=2),encoding='utf-8')
    print('Verified both loading movies: only script bytecode changed; art, single-tip timeline and native loading gauge preserved')


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--ffdec',type=Path,required=True)
    parser.add_argument('--java',default='java')
    build(parser.parse_args())
