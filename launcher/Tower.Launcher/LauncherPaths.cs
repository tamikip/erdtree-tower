using System;
using System.IO;
using System.Text;
using System.Windows.Forms;

namespace Tower.Launcher
{
    /// <summary>
    /// 启动器用到的目录：模组安装目录就是 exe 所在目录（更新也装在这里），依赖库与界面在 launcher/，
    /// 启动器自己的设置、日志与 WebView2 数据放 %LOCALAPPDATA%\ErdtreeTowerLauncher（不进安装目录，也不属于任何版本）。
    /// </summary>
    sealed class LauncherPaths
    {
        public string InstallDir { get; }
        public string LibDir => Path.Combine(InstallDir, "launcher");
        public string UiDir => Path.Combine(LibDir, "ui");
        /// <summary>下载线路配置（国内镜像地址），随模组发布、随更新替换</summary>
        public string SourcesFile => Path.Combine(LibDir, "sources.json");
        public string DataDir { get; }
        public string WebViewData => Path.Combine(DataDir, "WebView2");
        public string SettingsFile => Path.Combine(DataDir, "settings.json");
        public string LogFile => Path.Combine(DataDir, "launcher.log");
        public string LaunchErrorFile => Path.Combine(DataDir, "launch-error.txt");
        public string LaunchLogs => Path.Combine(DataDir, "launches");
        /// <summary>和 Start-Tower.ps1 共用：记住的游戏 Game 目录</summary>
        public string GamePathFile => Path.Combine(InstallDir, "GamePath.txt");

        public LauncherPaths()
        {
            InstallDir = Path.GetDirectoryName(Path.GetFullPath(Application.ExecutablePath))!;
            DataDir = LauncherData.Select(
                Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ErdtreeTowerLauncher"),
                Path.Combine(InstallDir, "launcher-data"));
        }

        /// <summary>只有启动器自己的文件（exe、依赖库）变了才需要重开启动器；界面文件重新加载页面即可</summary>
        public static bool IsLauncherBinary(string relative) =>
            relative.StartsWith("ErdtreeTowerLauncher.", StringComparison.OrdinalIgnoreCase) ||
            (relative.StartsWith("launcher/", StringComparison.OrdinalIgnoreCase) && !IsLauncherUi(relative));

        public static bool IsLauncherUi(string relative) => relative.StartsWith("launcher/ui/", StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>启动器日志：%LOCALAPPDATA%\ErdtreeTowerLauncher\launcher.log，超过 1 MB 轮换一次</summary>
    static class Log
    {
        static readonly object gate = new object();
        static string? path;

        public static void Init(string file)
        {
            path = file;
            try
            {
                var info = new FileInfo(file);
                if (info.Exists && info.Length > 1024 * 1024)
                {
                    File.Copy(file, file + ".old", true);
                    File.Delete(file);
                }
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        }

        public static void Write(string text)
        {
            if (path == null) return;
            lock (gate)
            {
                try { File.AppendAllText(path, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] {text}\r\n", Encoding.UTF8); }
                catch (IOException) { }
                catch (UnauthorizedAccessException) { }
            }
        }
    }
}
