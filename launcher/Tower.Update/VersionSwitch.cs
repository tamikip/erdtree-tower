using System;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;

namespace Tower.Update
{
    public static class VersionSwitch
    {
        public static int Build(string tag)
        {
            if (!Regex.IsMatch(tag, "^v[1-9][0-9]*$", RegexOptions.CultureInvariant) ||
                !int.TryParse(tag.Substring(1), out var build)) throw new InvalidDataException("无效的历史版本编号");
            return build;
        }

        public static ReleaseManifest Check(string tag, ReleaseManifest manifest)
        {
            if (manifest.Version != Build(tag) || manifest.Tag != tag) throw new InvalidDataException("历史版本清单与所选版本不一致");
            return manifest;
        }

        public static bool LauncherFile(string path) => path.StartsWith("launcher/", StringComparison.OrdinalIgnoreCase) ||
            path.Equals("ErdtreeTowerLauncher.exe", StringComparison.OrdinalIgnoreCase) ||
            path.Equals("ErdtreeTowerLauncher.exe.config", StringComparison.OrdinalIgnoreCase);

        public static UpdatePlan Plan(string installDir, ReleaseManifest target, HashCache cache, string launcherVersion)
        {
            var plan = Planner.Plan(installDir, target, cache);
            var installed = InstallState.Load(installDir) ?? throw new InvalidDataException("缺少安装记录，请先安装完整整合包");
            var kept = installed.Files.Where(f => LauncherFile(f.Path)).Select(f => {
                var path = InstallPaths.Full(installDir, f.Path);
                using var stream = File.OpenRead(path);
                return new FileEntry { Path = f.Path, Size = stream.Length, Sha256 = Hashes.Of(stream) };
            }).ToList();
            if (!kept.Any(f => f.Path.Equals("ErdtreeTowerLauncher.exe", StringComparison.OrdinalIgnoreCase)))
                throw new InvalidDataException("安装记录缺少启动器，不能安全切换版本");
            var effective = new ReleaseManifest {
                SchemaVersion = target.SchemaVersion, Version = target.Version, Tag = target.Tag,
                DisplayVersion = target.DisplayVersion, LauncherVersion = launcherVersion, UpdateKind = target.UpdateKind,
                Notes = target.Notes, Files = target.Files.Where(f => !LauncherFile(f.Path)).Concat(kept).ToList(), Blobs = target.Blobs
            };
            var writes = plan.Writes.Where(f => !LauncherFile(f.Path)).ToList();
            var needed = new System.Collections.Generic.HashSet<string>(writes.Select(f => f.Sha256));
            return new UpdatePlan(effective, plan.InstalledVersion, writes,
                plan.Deletes.Where(p => !LauncherFile(p)).ToList(),
                plan.Blobs.Where(b => needed.Contains(b.Key)).ToDictionary(b => b.Key, b => b.Value));
        }
    }
}
