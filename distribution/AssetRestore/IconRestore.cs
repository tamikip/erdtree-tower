using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;

namespace Tower.Assets
{
    internal static class IconRestore
    {
        public static void Restore(string package, string game, AssetManifest manifest, bool validation, Action<string> noLinks)
        {
            if (manifest.Icons == null || manifest.Icons.Length == 0) return;
            if (manifest.Icons.Length > 10000 || manifest.Icons.Any(i => i.Id < 0 || i.Id >= 990000) ||
                manifest.Icons.Select(i => i.Id).Distinct().Count() != manifest.Icons.Length) throw new InvalidDataException("Invalid icon manifest");
            var pending = new List<IconRecipe>();
            int checkedIcons = 0;
            Progress.Report("checking", 0, manifest.Icons.Length, "正在校验物品图标");
            string folder = Path.Combine(package, "scene-assets", "reward-icons");
            foreach (var icon in manifest.Icons)
            {
                string path = Path.Combine(folder, icon.Id + ".rgba"); noLinks(path);
                if (!File.Exists(path) || new FileInfo(path).Length != 102400 || AssetHash.FileHash(path) != icon.Sha256) pending.Add(icon);
                checkedIcons++;
                if (checkedIcons % 25 == 0 || checkedIcons == manifest.Icons.Length) Progress.Report("checking", checkedIcons, manifest.Icons.Length, "正在校验物品图标");
            }
            if (pending.Count == 0) { Console.WriteLine("物品图标校验通过。"); return; }
            if (!validation && Process.GetProcessesByName("eldenring").Length != 0) throw new IOException("请先保存退出游戏，再生成物品图标。");
            Console.WriteLine("正在从本地资源生成 " + pending.Count + " 张物品图标…");
            OodleDcx.Load(game, manifest.OodleHash);
            string atlasPath = Path.Combine(package, "scene-assets", "menu", "hi", "01_common.tpf.dcx");
            var expected = manifest.Files.Single(f => f.Path == "menu/hi/01_common.tpf.dcx").TargetHash;
            if (AssetHash.FileHash(atlasPath) != expected) throw new InvalidDataException("Icon source archive hash mismatch");
            var atlas = OodleDcx.Decode(File.ReadAllBytes(atlasPath));
            Directory.CreateDirectory(folder);
            int generated = 0;
            Progress.Report("icons", 0, pending.Count, "正在生成物品图标", "首次生成后会保存在本地");
            foreach (var icon in pending)
            {
                var bytes = IconCodec.Decode(atlas, icon.Offset, icon.Stride);
                if (AssetHash.Of(bytes) != icon.Sha256) throw new InvalidDataException("Icon pixels differ: " + icon.Id);
                string path = Path.Combine(folder, icon.Id + ".rgba"), temporary = path + ".tower-part";
                noLinks(temporary);
                try
                {
                    File.WriteAllBytes(temporary, bytes);
                    if (File.Exists(path)) File.Replace(temporary, path, null); else File.Move(temporary, path);
                }
                finally { if (File.Exists(temporary)) File.Delete(temporary); }
                generated++;
                if (generated % 10 == 0 || generated == pending.Count) Progress.Report("icons", generated, pending.Count, "正在生成物品图标", "已完成像素校验并保存");
            }
            Console.WriteLine("物品图标生成完成，像素校验通过。");
        }
    }
}
