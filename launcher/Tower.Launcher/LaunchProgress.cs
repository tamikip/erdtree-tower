using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;

namespace Tower.Launcher
{
    internal enum LaunchObservation { Waiting, Ready, Exited, Failed }
    internal sealed class LaunchLifetime
    {
        public bool GameSeen { get; private set; }
        int seenTick;
        public LaunchObservation Observe(bool gameRunning, bool frameReady, int tick)
        {
            if (!GameSeen && (gameRunning || frameReady)) { GameSeen = true; seenTick = tick; }
            if (!GameSeen) return LaunchObservation.Waiting;
            if (!gameRunning) return frameReady ? LaunchObservation.Exited : LaunchObservation.Failed;
            return frameReady || tick - seenTick >= 120 ? LaunchObservation.Ready : LaunchObservation.Waiting;
        }
    }

    internal sealed class LaunchProgress
    {
        readonly Stopwatch clock = Stopwatch.StartNew();
        bool nativeSession, gameSeen;
        public string Phase { get; private set; } = "preparing";
        public string Text { get; private set; } = "正在准备启动";
        public string Detail { get; private set; } = "检查游戏目录与启动配置";
        public int? Done { get; private set; }
        public int? Total { get; private set; }
        public bool Terminal { get; private set; }
        public bool FrameReady { get; private set; }
        public double? Percent => Total > 0 ? Done.GetValueOrDefault() * 100.0 / Total.Value : (double?)null;
        public int Seconds => (int)clock.Elapsed.TotalSeconds;

        public void Set(string phase, string text, string detail = "", int? done = null, int? total = null)
        {
            if (Terminal) return;
            Phase = phase; Text = text; Detail = detail; Done = done; Total = total;
        }
        public void End(string phase, string text, string detail = "")
        {
            Phase = phase; Text = text; Detail = detail; Done = Total = null; Terminal = true; clock.Stop();
        }
        public void GameDetected()
        {
            gameSeen = true;
            if (!nativeSession) Set("waiting", "游戏进程已启动，等待画面", "首次进入可能出现短暂黑屏，请勿重复启动");
        }
        public void Output(string line)
        {
            if (Terminal || nativeSession || gameSeen || !line.StartsWith("TOWER_PROGRESS|", StringComparison.Ordinal)) return;
            var p = line.Split(new[] { '|' }, 6);
            if (p.Length != 6 || !int.TryParse(p[2], out var done) || !int.TryParse(p[3], out var total)
                || done < 0 || total < 0 || done > total || p[4].Length > 200 || p[5].Length > 500) return;
            if (p[1] != "checking" && p[1] != "building" && p[1] != "icons" && p[1] != "starting") return;
            Set(p[1], p[4], p[5], total > 0 ? done : (int?)null, total > 0 ? total : (int?)null);
        }
        public void Native(string line)
        {
            if (Terminal) return;
            if (line.Contains("SESSION_START")) { nativeSession = true; Set("waiting", "正在加载爬塔模块", "游戏已启动，正在初始化界面"); }
            if (!nativeSession) return;
            if (line.Contains("NATIVE_SETTINGS registration")) Set("waiting", "正在初始化游戏界面", "原生设置已加载，等待渲染准备");
            if (line.Contains("DX12 renderer initialized")) Set("waiting", "正在准备首帧画面", "渲染器已就绪");
            if (line.Contains("First overlay frame rendered")) { FrameReady = true; End("finished", "游戏画面已开始绘制", "可以切换到游戏窗口继续"); }
        }
    }

    internal sealed class LaunchLogReader
    {
        readonly string path;
        long position;
        readonly Decoder decoder = Encoding.UTF8.GetDecoder();
        string pending = "";
        public LaunchLogReader(string path, bool skipExisting = false)
        {
            this.path = path;
            try { if (skipExisting && File.Exists(path)) position = new FileInfo(path).Length; }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        }
        public IEnumerable<string> Read()
        {
            var lines = new List<string>();
            try {
                using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                if (file.Length < position) { position = 0; pending = ""; decoder.Reset(); }
                file.Position = position;
                var bytes = new byte[65536]; var chars = new char[65536];
                int count = file.Read(bytes, 0, bytes.Length); position += count;
                pending += new string(chars, 0, decoder.GetChars(bytes, 0, count, chars, 0, false));
                int end;
                while ((end = pending.IndexOf('\n')) >= 0) { lines.Add(pending.Substring(0, end).TrimEnd('\r')); pending = pending.Substring(end + 1); }
                if (pending.Length > 8192) pending = "";
            } catch (IOException) { }
            catch (UnauthorizedAccessException) { }
            return lines;
        }
    }
}
