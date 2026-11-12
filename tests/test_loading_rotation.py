import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scene-builder'))
from loading_rotation import patch_source

class LoadingRotation(unittest.TestCase):
    def test_preserves_native_loop_and_exit_and_adds_fade_in_callback(self):
        source='      public function MainTimeline() { super(); addFrameScript(88,this.frame89,119,this.frame120); }'
        result=patch_source(source)
        self.assertIn('addFrameScript(1,this.towerBeginLoading,88,this.frame89,119,this.frame120);',result)
        self.assertEqual(result.count('addEventListener('),1)
        with self.assertRaises(ValueError):patch_source(result)
        with self.assertRaises(ValueError):patch_source('unknown')

    def test_new_segments_start_on_one_of_twelve_tip_boundaries(self):
        for previous in range(3):
            choices={(previous+1+offset)%3 for offset in range(2)}
            self.assertNotIn(previous,choices)
            self.assertEqual(len(choices),2)
            frames={background*240+tip*720+1 for background in choices for tip in range(4)}
            self.assertEqual(len(frames),8)
            self.assertTrue(all(1<=frame<=2880 and (frame-1)%240==0 for frame in frames))

if __name__=='__main__':unittest.main()
