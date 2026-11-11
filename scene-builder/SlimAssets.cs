using SoulsFormats;
using Tower.Assets;
using System.Text.Json;

static class SlimAssets
{
    public static void Build(string game, string output, string assets)
    {
        Directory.CreateDirectory(output);
        var manifest = new AssetManifest { Version = 1, OodleHash = AssetHash.FileHash(Path.Combine(game, "oo2core_6_win64.dll")) };
        var pending = new List<string> { "menu/hi/01_common.tpf.dcx", "sfx/sfxbnd_commoneffects.ffxbnd.dcx" };
        const string englishTheme="locales/en-US/menu/hi/01_common.tpf.dcx";
        if(File.Exists(Path.Combine(assets,englishTheme)))pending.Add(englishTheme);
        var recipes = new List<AssetRecipe>();
        foreach (var archive in new[] { "DLC", "Data0", "Data1", "Data2", "Data3" })
        {
            using var header = UXM.CryptographyUtility.DecryptRsa(Path.Combine(game, archive + ".bhd"), UXM.ArchiveKeys.EldenRingKeys[archive]);
            var entries = BHD5.Read(header, BHD5.Game.EldenRing).Buckets.SelectMany(b => b).ToDictionary(f => f.FileNameHash);
            using var data = File.OpenRead(Path.Combine(game, archive + ".bdt"));
            foreach (var path in pending.ToArray())
            {
                string sourcePath=path==englishTheme?"menu/hi/01_common.tpf.dcx":path;
                ulong hash = 0; foreach (char c in "/" + sourcePath) hash = unchecked(hash * 0x85 + c);
                if (!entries.TryGetValue(hash, out var entry)) continue;
                var original = entry.ReadFile(data);
                var raw = new byte[entry.PaddedFileSize]; data.Position = entry.FileOffset; data.ReadExactly(raw);
                var repairs = new List<SourceRepair>();
                for (int i = 0; i < raw.Length;)
                {
                    if (raw[i] == original[i]) { i++; continue; }
                    int start = i; while (i < raw.Length && raw[i] != original[i]) i++;
                    repairs.Add(new SourceRepair { Offset = start, Data = Convert.ToBase64String(original[start..i]) });
                }
                var target = File.ReadAllBytes(Path.Combine(assets, path));
                var decoded = DCX.Decompress(target, out var compression);
                if (compression is not DCX.DcxKrakCompressionInfo krak || krak.CompressionLevel != 6) throw new Exception("Unsupported target DCX");
                var source = DCX.Decompress(original);
                string patchName = path.Replace('/', '_') + ".patch", patch = Path.Combine(output, patchName);
                using (var stream = File.Create(patch)) AssetPatch.Create(source, decoded, stream);
                using (var stream = File.OpenRead(patch))
                    if (!AssetPatch.Apply(source, stream, decoded.Length).SequenceEqual(decoded)) throw new Exception("Patch round-trip failed");
                recipes.Add(new AssetRecipe {
                    Path = path, Archive = archive + ".bdt", Offset = entry.FileOffset, SourceSize = raw.Length, SourceHash = AssetHash.Of(raw),
                    Repairs = repairs.ToArray(), Patch = patchName, PatchHash = AssetHash.FileHash(patch), Header = Convert.ToBase64String(target[..76]),
                    DecodedSize = decoded.Length, DecodedHash = AssetHash.Of(decoded), TargetHash = AssetHash.Of(target)
                });
                Console.WriteLine($"{path}: patch {new FileInfo(patch).Length / 1048576.0:F2} MiB, exact decoded round-trip verified");
                pending.Remove(path);
            }
            if (pending.Count == 0) break;
        }
        if (pending.Count != 0) throw new Exception("Missing game resources: " + string.Join(", ", pending));
        manifest.Files = recipes.ToArray();
        manifest.Icons = SlimIcons.Build(game, assets, output);
        File.WriteAllText(Path.Combine(output, "manifest.json"), JsonSerializer.Serialize(manifest, new JsonSerializerOptions { WriteIndented = true }));
    }
}
