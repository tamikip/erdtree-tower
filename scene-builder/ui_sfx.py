"""界面音效：把 art-source/sfx-kenney/ 里的现成音效（Kenney 的 CC0 音效包，见该目录的 License.txt）
转成游戏播放用的 scene-assets/sfx/*.wav（44.1 kHz、16 位、单声道），并按用途统一响度。

用法：python scene-builder/ui_sfx.py <仓库根目录>
需要 ffmpeg（在 PATH 里）。只做格式转换与音量调整，不合成、不改音色。
- 每个音效先测平均响度（ffmpeg volumedetect），再增减到 CUES 里的目标值；峰值不超过 -1 dB
- 游戏里的总音量另由 tower-options.ini 的 sfx=0–100 控制（native/src/sfx.rs）
"""
from pathlib import Path
import re, subprocess, sys

# 游戏里的音效名 → （素材文件，目标平均响度 dB）
CUES = {
    "hover":   ("ui-audio_rollover2.ogg", -27.0),                 # 移动焦点、悬停：很轻
    "confirm": ("interface-sounds_confirmation_001.ogg", -18.0),  # 选定、确认、选中律法
    "close":   ("interface-sounds_back_002.ogg", -20.0),          # 取消、返回、取消律法
    "open":    ("interface-sounds_maximize_006.ogg", -22.0),      # 打开选择职业 / 结算页：低沉柔和，不刺耳
    "tick1":   ("interface-sounds_click_002.ogg", -26.0),         # 结算页分数滚动（三种轮流，选高频少的轻咔嗒）
    "tick2":   ("interface-sounds_click_003.ogg", -26.0),
    "tick3":   ("interface-sounds_click_005.ogg", -26.0),
    "lock1":   ("casino-audio_chips-stack-1.ogg", -21.0),         # 分数每一位停下
    "lock2":   ("casino-audio_chips-stack-2.ogg", -21.0),
    "lock3":   ("casino-audio_chips-stack-3.ogg", -21.0),
    "lock4":   ("casino-audio_chips-stack-4.ogg", -21.0),
    "lock5":   ("casino-audio_chips-stack-5.ogg", -21.0),
    "lock6":   ("casino-audio_chips-stack-6.ogg", -21.0),
    "sweep":   ("casino-audio_card-slide-2.ogg", -25.0),          # 高光扫过大数字
    "stamp":   ("impact-sounds_impactBell_heavy_002.ogg", -16.0), # 评级砸下
    "record":  ("interface-sounds_confirmation_004.ogg", -17.0),  # 新纪录
    "total":   ("interface-sounds_drop_002.ogg", -18.0),          # 旅程统计落定
}
PEAK = -1.0


def measure(path):
    out = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", str(path), "-ac", "1", "-af", "volumedetect", "-f", "null", "-"],
                         capture_output=True, text=True, check=True).stderr
    mean = float(re.search(r"mean_volume: (-?[\d.]+) dB", out).group(1))
    peak = float(re.search(r"max_volume: (-?[\d.]+) dB", out).group(1))
    return mean, peak


def main():
    root = Path(sys.argv[1])
    source, target = root / "art-source" / "sfx-kenney", root / "scene-assets" / "sfx"
    target.mkdir(parents=True, exist_ok=True)
    for name, (file, loudness) in CUES.items():
        path = source / file
        mean, peak = measure(path)
        gain = min(loudness - mean, PEAK - peak)
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(path), "-ac", "1", "-ar", "44100",
                        "-af", f"volume={gain:.2f}dB", "-c:a", "pcm_s16le", "-map_metadata", "-1", "-fflags", "+bitexact",
                        "-flags:a", "+bitexact", str(target / f"{name}.wav")], check=True)
        print(f"{name:8s} <- {file:42s} mean {mean:6.1f} dB  gain {gain:+5.1f} dB")


if __name__ == "__main__":
    main()
