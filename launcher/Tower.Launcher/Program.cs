using System;
using System.Diagnostics;
using System.Net;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;
using Tower.Update;

namespace Tower.Launcher
{
    static class Program
    {
        [STAThread]
        static void Main(string[] args)
        {
            if (args.Length == 1 && args[0] == "--lobby-worker") { LobbyClient.RunWorkerAsync().GetAwaiter().GetResult(); return; }
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            WaitForPrevious(args);
            // 同时只开一个：两个启动器同时更新同一个目录会互相破坏，第二个只负责把第一个的窗口叫到前面
            using var single = new Mutex(true, @"Local\ErdtreeTowerLauncher", out var first);
            if (!first)
            {
                var existing = FindWindow(null, MainForm.Title);
                if (existing != IntPtr.Zero) { ShowWindow(existing, 9 /* SW_RESTORE */); SetForegroundWindow(existing); }
                return;
            }
            var paths = new LauncherPaths();
            Log.Init(paths.LogFile);
            Log.Write($"启动：{paths.InstallDir}");
            Application.ThreadException += (s, e) => Crash(e.Exception);
            AppDomain.CurrentDomain.UnhandledException += (s, e) => Crash(e.ExceptionObject as Exception);
            // .NET Framework 默认每个主机只开 2 个连接，分段下载是 4 路并发
            ServicePointManager.DefaultConnectionLimit = 16;
            try { Updater.Recover(paths.InstallDir); }
            catch (Exception e) { Log.Write("还原上次中断的更新失败：" + e); }
            Application.Run(new MainForm(paths));
        }

        /// <summary>「立即重启」拉起的新进程带 --wait-for &lt;旧进程 ID&gt;：等旧的退出（最多 15 秒）再去拿“只开一个”的锁</summary>
        static void WaitForPrevious(string[] args)
        {
            var at = Array.IndexOf(args, "--wait-for");
            if (at < 0 || at + 1 >= args.Length || !int.TryParse(args[at + 1], out var id)) return;
            try
            {
                using var previous = Process.GetProcessById(id);
                previous.WaitForExit(15000);
            }
            catch (ArgumentException) { } // 已经退出了
            catch (InvalidOperationException) { }
        }

        static void Crash(Exception? e)
        {
            Log.Write("未处理的异常：" + e);
            MessageBox.Show($"启动器出错了：{e?.Message}\n\n详细信息已写入 %LOCALAPPDATA%\\ErdtreeTowerLauncher\\launcher.log", MainForm.Title, MessageBoxButtons.OK, MessageBoxIcon.Error);
        }

        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern IntPtr FindWindow(string? className, string windowName);
        [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr window, int command);
        [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr window);
    }
}
