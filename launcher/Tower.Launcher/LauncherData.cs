using System;
using System.IO;

namespace Tower.Launcher
{
    static class LauncherData
    {
        public static string Select(string preferred, string fallback)
        {
            if (Directory.Exists(fallback) && Writable(fallback)) return fallback;
            if (Writable(preferred)) return preferred;
            if (!Writable(fallback)) throw new IOException("启动器数据目录不可写，请将启动器移到可写的文件夹。");
            foreach (var name in new[] { "settings.json", "release-history.json" })
            {
                var source = Path.Combine(preferred, name);
                var target = Path.Combine(fallback, name);
                if (File.Exists(source) && !File.Exists(target)) File.Copy(source, target);
            }
            return fallback;
        }

        static bool Writable(string root)
        {
            try
            {
                foreach (var path in new[] { root, Path.Combine(root, "launches"), Path.Combine(root, "WebView2") })
                {
                    Directory.CreateDirectory(path);
                    using (var probe = new FileStream(Path.Combine(path, ".write-test-" + Guid.NewGuid().ToString("N")),
                        FileMode.CreateNew, FileAccess.Write, FileShare.None, 1, FileOptions.DeleteOnClose)) probe.WriteByte(0);
                }
                foreach (var name in new[] { "settings.json", "launcher.log", "launch-error.txt", "release-history.json" })
                {
                    var path = Path.Combine(root, name);
                    if (File.Exists(path)) using (File.Open(path, FileMode.Open, FileAccess.Write, FileShare.ReadWrite)) { }
                }
                return true;
            }
            catch (UnauthorizedAccessException) { return false; }
            catch (IOException) { return false; }
        }
    }
}
