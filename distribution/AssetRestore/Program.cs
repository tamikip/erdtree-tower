using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.Serialization.Json;
using System.Text;

namespace Tower.Assets
{
    internal static class Program
    {
        static readonly string[] Targets = { "menu/hi/01_common.tpf.dcx", "sfx/sfxbnd_commoneffects.ffxbnd.dcx" };
        static int Main(string[] args)
        {
            Console.OutputEncoding = new UTF8Encoding(false);
            try
            {
                bool validation = args.Length == 3 && args[2] == "--build-validation";
                if (args.Length != 2 && !validation) throw new ArgumentException("Usage: Tower.AssetRestore <package> <game>");
                string package = Path.GetFullPath(args[0]);
                if (validation && (File.Exists(Path.Combine(package, "preview.me3")) || Directory.GetFiles(package, "*.dll").Length != 0))
                    throw new IOException("Build validation requires an isolated asset-only directory");
                Restore(package, Path.GetFullPath(args[1]), validation); return 0;
            }
            catch (Exception error) { Console.Error.WriteLine("资源准备失败：" + error.Message); return 1; }
        }
        static void NoLinks(string path)
        {
            for (var item = new DirectoryInfo(Path.GetDirectoryName(path)); item != null; item = item.Parent)
                if (item.Exists && (item.Attributes & FileAttributes.ReparsePoint) != 0) throw new IOException("Resource path contains a junction: " + item.FullName);
            if (File.Exists(path) && (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) throw new IOException("Resource path contains a link");
        }
        static void Restore(string package, string game, bool validation)
        {
            string folder = Path.Combine(package, "asset-patches"), manifestPath = Path.Combine(folder, "manifest.json");
            if (!File.Exists(manifestPath)) return;
            NoLinks(manifestPath);
            using (var guard = new FileStream(Path.Combine(folder, ".restore.lock"), FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None))
            {
                AssetManifest manifest;
                using (var stream = File.OpenRead(manifestPath)) manifest = (AssetManifest)new DataContractJsonSerializer(typeof(AssetManifest)).ReadObject(stream);
                var paths=manifest.Files==null?new HashSet<string>():new HashSet<string>(manifest.Files.Select(f=>f.Path));
                var allowed=Targets.Concat(new[]{"locales/en-US/menu/hi/01_common.tpf.dcx"});
                if (manifest.Version != 1 || manifest.Files == null || paths.Count!=manifest.Files.Length ||
                    !paths.IsSupersetOf(Targets)||!paths.IsSubsetOf(allowed)) throw new InvalidDataException("Invalid asset manifest");
                var pending = new List<AssetRecipe>();
                foreach (var file in manifest.Files)
                {
                    string target = Path.Combine(package, "scene-assets", file.Path); NoLinks(target);
                    if (!File.Exists(target) || AssetHash.FileHash(target) != file.TargetHash) pending.Add(file);
                }
                if (pending.Count == 0) { Console.WriteLine("本地资源校验通过。"); IconRestore.Restore(package, game, manifest, validation, NoLinks); return; }
                if (!validation && Process.GetProcessesByName("eldenring").Length != 0) throw new IOException("请先保存退出游戏，再修复资源。");
                OodleDcx.Load(game, manifest.OodleHash);
                var staged = new List<Tuple<string, string>>();
                try
                {
                    foreach (var file in pending)
                    {
                        Console.WriteLine("正在本地生成 " + file.Path + "（首次可能需要数分钟）");
                        if (!new[] { "Data0.bdt", "Data1.bdt", "Data2.bdt", "Data3.bdt", "DLC.bdt" }.Contains(file.Archive) ||
                            file.SourceSize <= 0 || file.SourceSize > 512 * 1024 * 1024 || file.Offset < 0 ||
                            file.Patch != file.Path.Replace('/', '_') + ".patch") throw new InvalidDataException("Invalid source recipe");
                        var source = new byte[file.SourceSize];
                        using (var input = File.OpenRead(Path.Combine(game, file.Archive)))
                        {
                            if (file.Offset > input.Length - source.Length) throw new InvalidDataException("游戏资源版本不匹配，请使用完整整合包。");
                            input.Position = file.Offset; int offset = 0;
                            while (offset < source.Length) { int count = input.Read(source, offset, source.Length - offset); if (count == 0) throw new EndOfStreamException(); offset += count; }
                        }
                        if (AssetHash.Of(source) != file.SourceHash) throw new InvalidDataException("游戏资源版本不匹配或已被修改，请使用完整整合包：" + file.Path);
                        foreach (var repair in file.Repairs)
                        {
                            var bytes = Convert.FromBase64String(repair.Data);
                            if (repair.Offset < 0 || repair.Offset > source.Length - bytes.Length) throw new InvalidDataException("Invalid source repair");
                            Buffer.BlockCopy(bytes, 0, source, repair.Offset, bytes.Length);
                        }
                        source = OodleDcx.Decode(source);
                        string patch = Path.Combine(folder, file.Patch); NoLinks(patch);
                        if (AssetHash.FileHash(patch) != file.PatchHash) throw new InvalidDataException("补丁损坏，请在启动器校验并修复文件。");
                        byte[] decoded;
                        using (var stream = File.OpenRead(patch)) decoded = AssetPatch.Apply(source, stream, file.DecodedSize);
                        if (AssetHash.Of(decoded) != file.DecodedHash) throw new InvalidDataException("Asset patch verification failed");
                        source = null;
                        var result = OodleDcx.Encode(decoded, Convert.FromBase64String(file.Header));
                        if (AssetHash.Of(result) != file.TargetHash) throw new InvalidDataException("生成资源校验失败，原文件未替换。");
                        string target = Path.Combine(package, "scene-assets", file.Path), temporary = target + ".tower-part";
                        NoLinks(temporary); Directory.CreateDirectory(Path.GetDirectoryName(target));
                        File.WriteAllBytes(temporary, result); staged.Add(Tuple.Create(temporary, target));
                        GC.Collect();
                    }
                    // All outputs pass their release hashes before replacing any installed asset.
                    foreach (var item in staged)
                    {
                        if (File.Exists(item.Item2)) File.Replace(item.Item1, item.Item2, null);
                        else File.Move(item.Item1, item.Item2);
                    }
                    Console.WriteLine("本地资源生成完成，校验通过。");
                    IconRestore.Restore(package, game, manifest, validation, NoLinks);
                }
                finally { foreach (var item in staged) if (File.Exists(item.Item1)) File.Delete(item.Item1); }
            }
        }
    }
}
