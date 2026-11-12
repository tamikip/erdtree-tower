using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;
using Microsoft.Win32;
using Tower.Update;

namespace Tower.Launcher
{
    /// <summary>
    /// 页面指令的实现：安装/更新（Tower.Update，只下载内容有变化的文件）、启动游戏（Start-Tower.ps1）、设置与本地功能。
    /// 页面里只有一个“游戏”：tower，也就是启动器所在文件夹里的模组。除下载线程报进度外，都在界面线程上运行。
    /// </summary>
    sealed class TowerHost : IDisposable
    {
        const string Game = "tower";
        const string Repo = "https://github.com/tamikip/erdtree-tower";

        readonly LauncherPaths paths;
        readonly MainForm form;
        readonly HostSettings settings;
        readonly HttpClient http;
        readonly string? mirror;
        readonly Plugins plugins;
        MultiSource source;

        ReleaseManifest? latest;
        string updateStatus = "尚未检查";
        bool checking, launching;
        LaunchProgress? launchProgress;
        bool historyLoading;
        int? launchWorkerId;
        CancellationTokenSource? running;
        bool pausing, lastVerify;
        /// <summary>这一轮下载（含暂停后继续、失败后重试）的总字节数：已经下好的部分算作完成，进度不会倒退</summary>
        long sessionTotal = -1;
        volatile bool installing;
        /// <summary>界面文件更新后会重新加载页面，完成提示留到页面重新就绪时再显示</summary>
        (string text, int? ms, bool restart)? pendingToast;

        public bool Installing => installing;

        public TowerHost(LauncherPaths paths, MainForm form)
        {
            this.paths = paths;
            this.form = form;
            settings = HostSettings.Load(paths.SettingsFile);
            // 跳转由 HttpReleaseSource 自己跟，保证每一跳都带 Range
            http = new HttpClient(new HttpClientHandler { AllowAutoRedirect = false }) { Timeout = TimeSpan.FromMinutes(5) };
            http.DefaultRequestHeaders.UserAgent.ParseAdd($"ErdtreeTowerLauncher/{typeof(TowerHost).Assembly.GetName().Version}");
            mirror = ReadMirror(paths.SourcesFile);
            plugins = new Plugins(paths.InstallDir);
            source = CreateSource();
        }

        /// <summary>
        /// 下载线路：国内镜像（launcher/sources.json 配了才有）与 GitHub Release，按设置决定谁优先，另一条兜底。
        /// 环境变量 ERDTREE_TOWER_UPDATE_SOURCE 可改成只用某个网址或本地目录（发版前自测用）。
        /// </summary>
        MultiSource CreateSource()
        {
            var custom = Environment.GetEnvironmentVariable("ERDTREE_TOWER_UPDATE_SOURCE");
            if (!string.IsNullOrWhiteSpace(custom))
            {
                Log.Write("更新源：" + custom);
                IReleaseSource only = Directory.Exists(custom) ? new DirectoryReleaseSource(custom!) : new HttpReleaseSource(http, custom!);
                return new MultiSource(new[] { new KeyValuePair<string, IReleaseSource>("自定义", only) });
            }
            var lines = new List<KeyValuePair<string, IReleaseSource>> { new KeyValuePair<string, IReleaseSource>("GitHub", HttpReleaseSource.GitHub(http, "tamikip", "erdtree-tower")) };
            if (mirror != null)
                lines.Insert(settings.Route == HostSettings.GitHubFirst ? 1 : 0, new KeyValuePair<string, IReleaseSource>("国内镜像", new HttpReleaseSource(http, mirror)));
            return new MultiSource(lines);
        }

        /// <summary>sources.json 里的国内镜像地址，只接受 https；没配或写错返回 null（只用 GitHub）</summary>
        static string? ReadMirror(string file)
        {
            try
            {
                if (!File.Exists(file)) return null;
                using var document = JsonDocument.Parse(File.ReadAllText(file));
                var url = Text(document.RootElement, "mirror").Trim();
                if (url.Length == 0) return null;
                if (Uri.TryCreate(url, UriKind.Absolute, out var uri) && uri.Scheme == Uri.UriSchemeHttps) return url;
                Log.Write("sources.json 的镜像地址不是 https 网址，已忽略：" + url);
            }
            catch (Exception e) when (e is IOException || e is JsonException || e is UnauthorizedAccessException) { Log.Write("读取 sources.json 失败：" + e.Message); }
            return null;
        }

        public void Dispose()
        {
            running?.Cancel();
            http.Dispose();
        }

        // ---------------- 页面 → 宿主 ----------------

        public void Handle(string json)
        {
            string type, action;
            JsonElement payload;
            try
            {
                using var document = JsonDocument.Parse(json);
                var root = document.RootElement;
                type = Text(root, "type");
                action = Text(root, "action");
                payload = root.ValueKind == JsonValueKind.Object && root.TryGetProperty("payload", out var p) ? p.Clone() : default;
            }
            catch (JsonException) { return; }
            try { Dispatch(type, action, payload); }
            catch (Exception e)
            {
                Log.Write($"处理 {type}.{action} 出错：{e}");
                Toast("出错了：" + e.Message, "error", 6000);
            }
        }

        void Dispatch(string type, string action, JsonElement payload)
        {
            switch (type + "." + action)
            {
                case "window.minimize": form.WindowState = FormWindowState.Minimized; break;
                case "window.close": case "app.quit": form.Close(); break;
                case "window.startDrag": form.StartDrag(); break;
                case "app.queryHost": form.Post("app", "hostTakeover", new { download = true }); break;
                case "app.checkUpdate": _ = CheckAsync(true); break;
                case "game.queryStatus": PushStatus(); break;
                case "game.download": _ = UpdateAsync(false); break;
                case "game.resume": _ = UpdateAsync(lastVerify); break;
                case "game.pause": Stop(true); break;
                case "game.cancel": Stop(false); break;
                case "game.launch": _ = LaunchAsync(); break;
                case "lobby.list": _ = LobbyListAsync(); break;
                case "lobby.host": _ = LobbyHostAsync(payload); break;
                case "lobby.join": _ = LobbyJoinAsync(payload); break;
                case "lobby.close": CloseLobby(); break;
                case "game.locate": OpenFolder(paths.InstallDir); break;
                case "settings.change":
                    Change(Text(payload, "key"), payload.ValueKind == JsonValueKind.Object && payload.TryGetProperty("value", out var value) ? value : default);
                    break;
                case "settings.action":
                    Run(Text(payload, "name"), payload.ValueKind == JsonValueKind.Object && payload.TryGetProperty("payload", out var data) ? data : default);
                    break;
                case "shell.openExternal": OpenUrl(Text(payload, "url")); break;
            }
        }

        static string Text(JsonElement element, string name) =>
            element.ValueKind == JsonValueKind.Object && element.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String ? value.GetString() ?? "" : "";

        void Change(string key, JsonElement value)
        {
            // 调试与改键写进模组目录的 tower-options.ini（游戏读它），不进启动器自己的设置
            if (key.StartsWith("ersc.", StringComparison.Ordinal))
            {
                ChangeSeamless(key.Substring(5), value);
                return;
            }
            if (key == "debug" || key == "dlc" || key == "prompts" || key == "language" || key.StartsWith("key.", StringComparison.Ordinal))
            {
                ChangeOption(key, value);
                return;
            }
            switch (key)
            {
                case "loader":
                    settings.Loader = value.ValueKind == JsonValueKind.String && value.GetString() == HostSettings.Yafsml ? HostSettings.Yafsml : HostSettings.Me3;
                    settings.Save(paths.SettingsFile);
                    PushStatus();
                    return;
                case "mode":
                    settings.Mode = HostSettings.Seamless;
                    settings.Save(paths.SettingsFile);
                    PushStatus();  // 首页主按钮下面的模式提示跟着变
                    return;
                case "autoCheck": settings.AutoCheck = value.ValueKind == JsonValueKind.True; break;
                case "closeExit": settings.CloseExit = value.ValueKind == JsonValueKind.True; break;
                case "route":
                    settings.Route = value.ValueKind == JsonValueKind.String && value.GetString() == "GitHub" ? HostSettings.GitHubFirst : HostSettings.MirrorFirst;
                    source = CreateSource();
                    break;
                default: return;
            }
            settings.Save(paths.SettingsFile);
        }

        string OptionsFile => Path.Combine(paths.InstallDir, TowerOptions.FileName);

        // ---------------- 无缝联机 ----------------

        SeamlessSettings? Seamless() => SeamlessSettings.ForPackage(paths.InstallDir);

        (string path, long size, DateTime time, string label)? versionCache;

        /// <summary>版本要算 ersc.dll 的哈希：文件没变就用上次的结果</summary>
        string SeamlessLabel(SeamlessSettings? seamless)
        {
            if (seamless == null) return "先在「游戏」里设置游戏目录";
            var info = new FileInfo(seamless.Dll);
            if (info.Exists && versionCache is { } cache && cache.path == info.FullName && cache.size == info.Length && cache.time == info.LastWriteTimeUtc) return cache.label;
            string label;
            try { label = seamless.Version().Label; }
            catch (Exception e) when (e is IOException || e is UnauthorizedAccessException) { label = "读取失败：" + e.Message; }
            if (info.Exists) versionCache = (info.FullName, info.Length, info.LastWriteTimeUtc, label);
            return label;
        }

        void ChangeSeamless(string key, JsonElement value)
        {
            if (key == SeamlessSettings.Password && !CanPrepareLaunch()) return;
            var seamless = Seamless();
            if (seamless == null) { Toast("先在「游戏」里设置游戏目录", "warn"); return; }
            var text = value.ValueKind switch
            {
                JsonValueKind.True => "1",
                JsonValueKind.False => "0",
                JsonValueKind.String => value.GetString() ?? "",
                _ => value.ToString(),
            };
            try
            {
                seamless.Write(key, text);
                Log.Write($"无缝联机设置：{key}{(key == SeamlessSettings.Password ? "（已修改）" : " = " + text)}");
                Toast(key == SeamlessSettings.Password ? "联机密码已保存，和队友填同一个" : "已保存" + (GameRunning() ? "，下次启动游戏生效" : ""), "success");
            }
            catch (Exception e) when (e is InvalidOperationException || e is IOException || e is UnauthorizedAccessException)
            {
                Toast(e.Message, "error", 6000);
            }
            PushValues(InstallState.Load(paths.InstallDir));
        }

        void AddSeamlessValues(Dictionary<string, object?> values)
        {
            var seamless = Seamless();
            values["erscInfo"] = SeamlessLabel(seamless);
            var ini = seamless?.Read() ?? new Dictionary<string, string>();
            values["ersc." + SeamlessSettings.Password] = ini.TryGetValue(SeamlessSettings.Password, out var password) ? password : "";
            foreach (var key in SeamlessSettings.Switches) values["ersc." + key] = ini.TryGetValue(key, out var on) && on == "1";
            foreach (var key in SeamlessSettings.Scalings) values["ersc." + key] = ini.TryGetValue(key, out var percent) ? percent : "（无此项）";
        }

        // ---------------- 额外 DLL 插件 ----------------

        void AddPlugins(bool folder)
        {
            var answer = MessageBox.Show(form, "插件是第三方程序，会随游戏一起运行，可以做任何事。\n只添加你信任的来源；插件和无缝联机、爬塔之间的兼容性也无法保证。\n\n继续添加？",
                MainForm.Title, MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
            if (answer != DialogResult.Yes) return;
            var added = new List<string>();
            try
            {
                if (folder)
                {
                    using var dialog = new FolderBrowserDialog { Description = "选择插件自己的文件夹（顶层要有 .dll）", ShowNewFolderButton = false };
                    if (dialog.ShowDialog(form) != DialogResult.OK) return;
                    added.AddRange(plugins.AddFolder(dialog.SelectedPath));
                }
                else
                {
                    using var dialog = new OpenFileDialog { Title = "选择插件 DLL", Filter = "DLL 插件 (*.dll)|*.dll", Multiselect = true };
                    if (dialog.ShowDialog(form) != DialogResult.OK) return;
                    foreach (var file in dialog.FileNames) added.Add(plugins.AddDll(file));
                }
            }
            catch (Exception e) when (e is InvalidOperationException || e is IOException || e is UnauthorizedAccessException)
            {
                Toast("添加插件失败：" + e.Message, "error", 6000);
            }
            if (added.Count > 0)
            {
                Log.Write("添加插件：" + string.Join(", ", added));
                PluginsChanged($"已添加 {added.Count} 个插件" + (added.Count > 1 ? "；文件夹里的辅助 DLL 不需要加载的话，在列表里关掉" : ""));
            }
            else PluginsChanged(null);
        }

        void RemovePlugin(string path)
        {
            if (path.Length == 0) return;
            if (MessageBox.Show(form, $"移除插件 {path}？\n复制进模组 plugins 文件夹的文件也会删除。", MainForm.Title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
            try { plugins.Remove(path); }
            catch (Exception e) when (e is IOException || e is UnauthorizedAccessException) { Toast("移除失败（游戏还在运行？）：" + e.Message, "error", 6000); }
            PluginsChanged("已移除");
        }

        void PluginsChanged(string? message)
        {
            PushValues(InstallState.Load(paths.InstallDir));
            if (message != null) Toast(message + (GameRunning() ? "，下次启动游戏生效" : ""), "success");
            else if (GameRunning()) Toast("下次启动游戏生效", "info");
        }

        void ChangeOption(string key, JsonElement value)
        {
            var hotkey = TowerOptions.Hotkeys.FirstOrDefault(h => "key." + h.Id == key);
            if (key != "debug" && key != "dlc" && key != "prompts" && key != "language" && hotkey.Id == null) return;
            var label = value.ValueKind == JsonValueKind.String ? value.GetString() ?? "" : "";
            var text = key == "debug" ? (value.ValueKind == JsonValueKind.True ? "1" : "0")
                : key == "dlc" ? TowerOptions.DlcModes.FirstOrDefault(m => m.Label == label).Id ?? ""
                : key == "prompts" ? TowerOptions.PromptModes.FirstOrDefault(m => m.Label == label).Id ?? ""
                : key == "language" ? TowerOptions.Languages.FirstOrDefault(m => m.Label == label).Id ?? ""
                : label;
            if (text.Length == 0) return;
            try
            {
                TowerOptions.Write(OptionsFile, key, text);
                // 关调试时连旧的 tower-debug.enable 标记文件一起删，否则游戏里仍然算开着
                var flag = Path.Combine(paths.InstallDir, "tower-debug.enable");
                if (key == "debug" && text == "0" && File.Exists(flag)) File.Delete(flag);
            }
            catch (Exception e) when (e is IOException || e is UnauthorizedAccessException)
            {
                Toast("保存启动参数失败：" + e.Message, "error", 6000);
                return;
            }
            if (hotkey.Id != null)
            {
                // 两个功能绑同一个键时两个都会触发：提醒一下，但不拦
                var values = TowerOptions.Read(OptionsFile);
                var clash = TowerOptions.Hotkeys.Where(h => h.Id != hotkey.Id && TowerOptions.Key(values, h.Id, h.Default).Equals(text, StringComparison.OrdinalIgnoreCase)).Select(h => h.Label).ToList();
                if (clash.Count > 0) Toast($"「{hotkey.Label}」和「{string.Join("」「", clash)}」都是 {text}，按一下会同时触发", "warn", 6000);
            }
            if (key == "dlc" && text != "full") Toast("已改为没有 DLC：单人不出 DLC 内容；联机时队伍里有人没有 DLC，整队都按没有 DLC 运行", "info", 6000);
            else if (GameRunning()) Toast("已保存，下次启动游戏生效", "info");
        }

        void Run(string name, JsonElement data = default)
        {
            switch (name)
            {
                case "addPluginDll": AddPlugins(false); break;
                case "addPluginFolder": AddPlugins(true); break;
                case "openPlugins": Directory.CreateDirectory(plugins.Dir); OpenFolder(plugins.Dir); break;
                case "togglePlugin":
                    plugins.SetEnabled(Text(data, "path"), data.ValueKind == JsonValueKind.Object && data.TryGetProperty("enabled", out var on) && on.ValueKind == JsonValueKind.True);
                    PluginsChanged(null);
                    break;
                case "removePlugin": RemovePlugin(Text(data, "path")); break;
                case "seamlessDownload": OpenUrl("https://www.nexusmods.com/eldenring/mods/510"); break;
                case "seamlessFolder":
                    if (Seamless() is { } folder && Directory.Exists(folder.Folder)) OpenFolder(folder.Folder);
                    else Toast("包内无缝组件缺失，请校验并修复文件", "warn");
                    break;
                case "seamlessIni":
                    if (Seamless() is { } ini && File.Exists(ini.IniFile)) OpenDocument(ini.IniFile, "https://www.nexusmods.com/eldenring/mods/510");
                    else Toast("没找到无缝联机的配置文件 ersc_settings.ini", "warn");
                    break;
                case "checkUpdate": _ = CheckAsync(true); break;
                case "verify": _ = UpdateAsync(true); break;
                case "chooseGameDir": ChooseGameDir(); break;
                case "openGuide": OpenDocument(PackageDocument("安装教程.md", "安装教程.txt"), Repo + "#readme"); break;
                case "openWiki": OpenWiki(); break;
                case "openLicenses": OpenDocument(PackageDocument("THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"), Repo + "/tree/main/licenses"); break;
                case "openLogs": OpenLogs(); break;
                case "openModLogs": OpenModLogs(); break;
                case "releaseHistory": _ = HistoryAsync(); break;
                case "restartLauncher": Restart(); break;
            }
        }

        // ---------------- 宿主 → 页面 ----------------

        public void PageReady()
        {
            form.Post("app", "hostTakeover", new { download = true });
            PushStatus();
            if (pendingToast is { } toast) { pendingToast = null; Done(toast.text, toast.ms, toast.restart); }
            PushLaunchProgress();
            if (settings.AutoCheck && latest == null) _ = CheckAsync(false);
        }

        /// <summary>页面提示；button/then 给提示加一个按钮，点了以 settings.action(then) 回到宿主</summary>
        public void Toast(string text, string kind = "info", int? ms = null, string? button = null, string? then = null) =>
            form.Post("app", "toast", new { text, kind, ms, button, then });

        void Done(string text, int? ms, bool restart)
        {
            if (restart) Toast(text, "success", 0, "立即重启", "restartLauncher");
            else Toast(text, "success", ms);
        }

        void PushStatus()
        {
            var state = InstallState.Load(paths.InstallDir);
            var installed = IsInstalled(state);
            var updateTo = installed && latest != null && (state == null || latest.Version > state.Version) ? Label(latest) : null;
            var note = "统一入口 · 内置无缝联机";
            note += " · " + settings.Loader;
            form.Post("game", "status", new { game = Game, installed, version = PackageVersion(state), updateTo, note });
            PushValues(state);
        }

        void PushValues(InstallState? state)
        {
            var options = TowerOptions.Read(OptionsFile);
            var values = new Dictionary<string, object?>
            {
                ["debug"] = TowerOptions.Debug(options) || File.Exists(Path.Combine(paths.InstallDir, "tower-debug.enable")),
                ["dlc"] = TowerOptions.DlcLabel(options),
                ["prompts"] = TowerOptions.PromptLabel(options),
                ["language"] = TowerOptions.LanguageLabel(options),
                ["plugins"] = plugins.List().Select(p => new Dictionary<string, object?> { ["path"] = p.Path, ["name"] = p.Name, ["enabled"] = p.Enabled, ["exists"] = p.Exists }).ToList(),
                ["mode"] = settings.Mode == HostSettings.Seamless ? "无缝联机" : "单人",
                ["loader"] = settings.Loader,
                ["route"] = settings.Route == HostSettings.GitHubFirst ? "GitHub" : "国内镜像",
                ["mirrorInfo"] = mirror == null ? "未配置（只从 GitHub 更新）" : new Uri(mirror).Host,
                ["autoCheck"] = settings.AutoCheck,
                ["closeExit"] = settings.CloseExit,
                ["gameDir"] = GameDir() ?? "（未设置：开始游戏时自动查找，找不到再询问）",
                ["version"] = PackageVersion(state),
                ["launcherVersion"] = LauncherVersionLabel(state),
                ["updateStatus"] = updateStatus,
            };
            foreach (var (id, _, fallback) in TowerOptions.Hotkeys) values["key." + id] = TowerOptions.Key(options, id, fallback);
            AddSeamlessValues(values);
            form.Post("settings", "values", values);
        }

        /// <summary>页面从完整 Markdown 提取玩家更新要点。</summary>
        void PushNotes(ReleaseManifest release)
        {
            form.Post("game", "news", new { game = Game, tab = "notice", notes = release.Notes, version = Label(release) });
        }

        void SetStatus(string text)
        {
            updateStatus = text;
            PushValues(InstallState.Load(paths.InstallDir));
        }

        void Progress(string kind, string phase, double? percent = null, long? total = null, double? speed = null, string? error = null, bool? paused = null)
        {
            var payload = new Dictionary<string, object?> { ["game"] = Game, ["kind"] = kind, ["phase"] = phase };
            if (percent != null) payload["percent"] = Math.Round(Math.Min(100, percent.Value), 1);
            if (total != null) payload["total"] = total;
            if (speed != null) payload["speed"] = Math.Round(speed.Value);
            if (error != null) payload["error"] = error;
            if (paused != null) payload["paused"] = paused;
            form.Post("game", "progress", payload);
        }

        bool IsInstalled(InstallState? state) => state != null || File.Exists(Path.Combine(paths.InstallDir, "Start-Tower.ps1"));

        static string Label(ReleaseManifest release) => release.UpdateKind == "launcher" && release.LauncherVersion.Length > 0
            ? "启动器 " + release.LauncherVersion : Versions.PlayerLabel(release.DisplayVersion);

        static string LauncherVersionLabel(InstallState? state)
        {
            var current = typeof(TowerHost).Assembly.GetName().Version?.ToString(3) ?? "未知";
            return !string.IsNullOrEmpty(state?.LauncherVersion) && state!.LauncherVersion != current
                ? current + "（已安装 " + state.LauncherVersion + "，重启生效）" : current;
        }
        static string Label(InstallState state) => Versions.PlayerLabel(state.DisplayVersion);
        string PackageVersion(InstallState? state) => state != null ? Label(state) : IsInstalled(state) ? "开发版（未发布）" : "未安装";

        // ---------------- 检查与安装更新 ----------------

        async Task CheckAsync(bool user)
        {
            if (checking) return;
            checking = true;
            SetStatus("正在检查…");
            try
            {
                var lines = source;
                latest = await lines.LatestAsync(CancellationToken.None);
                var state = InstallState.Load(paths.InstallDir);
                var newer = state == null || latest.Version > state.Version;
                SetStatus(newer ? $"发现新版本 {Label(latest)}（{lines.Chosen}）" : $"已是最新版本（{Label(latest)} · {lines.Chosen}）");
                PushNotes(latest);
                PushStatus();
                if (newer && IsInstalled(state)) Toast($"发现新版本 {Label(latest)}：点「立即更新」，只下载有变化的文件", "info", 6000);
                else if (user && !newer) Toast("已是最新版本", "success");
            }
            catch (Exception e)
            {
                Log.Write("检查更新失败：" + e);
                SetStatus("检查失败：" + Describe(e));
                if (user) Toast("检查更新失败：" + Describe(e), "error", 6000);
            }
            finally { checking = false; }
        }

        /// <summary>
        /// 安装、更新、校验修复都是同一件事：拿最新清单和本地文件逐个比哈希，只下载缺的和不一样的。
        /// verify 时不信任哈希缓存，每个文件都重算。暂停/取消即取消下载，已经下好并校验过的数据留在暂存区，继续时跳过。
        /// </summary>
        async Task UpdateAsync(bool verify)
        {
            if (running != null) return;
            if (launching || LocalAssetPreparationBusy()) { Toast("正在启动游戏或生成本地资源，完成后再更新", "warn"); return; }
            var kind = IsInstalled(InstallState.Load(paths.InstallDir)) ? "update" : "install";
            if (GameRunning())
            {
                Progress(kind, "failed", error: "请先退出艾尔登法环再更新（游戏正在使用模组文件）");
                return;
            }
            var cancel = new CancellationTokenSource();
            running = cancel;
            pausing = false;
            lastVerify = verify;
            try
            {
                Progress(kind, "verifying");
                // 这一轮固定用同一组线路：中途改设置不影响正在进行的下载
                var lines = source;
                var target = await lines.LatestAsync(cancel.Token);
                latest = target;
                var plan = await Task.Run(() =>
                {
                    var cache = HashCache.Load(paths.InstallDir);
                    if (verify) cache.Clear();
                    return Planner.Plan(paths.InstallDir, target, cache);
                }, cancel.Token);
                cancel.Token.ThrowIfCancellationRequested();
                if (plan.UpToDate)
                {
                    sessionTotal = -1;
                    Progress(kind, "finished");
                    SetStatus($"已是最新版本（{Label(target)} · {lines.Chosen}）");
                    PushStatus();
                    var generatedAssets = UsesLocalAssets();
                    Toast(verify ? (generatedAssets ? "下载文件校验完成；本地生成资源将在启动前检查并修复" : "校验完成：文件都完整") : "已是最新版本", "success");
                    return;
                }
                if (sessionTotal < plan.DownloadBytes) sessionTotal = plan.DownloadBytes;
                var reporter = new Reporter(this, kind, sessionTotal - plan.DownloadBytes, sessionTotal, cancel.Token);
                Log.Write($"{(verify ? "校验" : "更新")}到 {Label(target)}（{lines.Chosen}）：写入 {plan.Writes.Count} 个文件，删除 {plan.Deletes.Count} 个，下载 {Size(plan.DownloadBytes)}");
                await Task.Run(() => new Updater(paths.InstallDir, lines).ApplyAsync(plan, reporter, cancel.Token), cancel.Token);
                sessionTotal = -1;
                Progress(kind, "finished", 100);
                SetStatus($"已是最新版本（{Label(target)} · {lines.Chosen}）");
                PushStatus();
                Finished(plan, kind, verify);
            }
            catch (OperationCanceledException) when (cancel.IsCancellationRequested)
            {
                if (pausing) Progress(kind, "paused", paused: true);
            }
            catch (Exception e)
            {
                Log.Write("更新失败：" + e);
                Progress(kind, "failed", error: Describe(e));
            }
            finally
            {
                installing = false;
                running = null;
                cancel.Dispose();
            }
        }

        void Finished(UpdatePlan plan, string kind, bool verify)
        {
            var version = Label(plan.Target);
            var text = verify ? $"校验完成：修复了 {plan.Writes.Count} 个文件（下载 {Size(plan.DownloadBytes)}）"
                : kind == "install" ? $"安装完成：{version}"
                : $"已更新到 {version}，只下载了 {Size(plan.DownloadBytes)}";
            var restart = plan.Writes.Any(w => LauncherPaths.IsLauncherBinary(w.Path));
            if (UsesLocalAssets()) text += "。本地资源将在启动前生成或修复";
            if (restart) text += "。启动器本身也更新了，重启后生效";
            int? ms = restart ? 0 : 6000;
            if (plan.Writes.Any(w => LauncherPaths.IsLauncherUi(w.Path)))
            {
                // 界面文件换了：重新加载页面，提示等页面就绪后再显示
                pendingToast = (text, ms, restart);
                _ = ReloadSoonAsync();
                return;
            }
            Done(text, ms, restart);
        }

        /// <summary>
        /// 重开启动器：先拉起新进程（带上本进程 ID，它会等这边退出后再接手“只开一个”的锁），再关掉自己。
        /// 正在下载或安装时不重启，免得打断。
        /// </summary>
        void Restart()
        {
            if (running != null) { Toast("正在更新，完成后再重启", "warn"); return; }
            Log.Write("重启启动器");
            Process.Start(new ProcessStartInfo(Application.ExecutablePath, $"--wait-for {Process.GetCurrentProcess().Id}") { UseShellExecute = false, WorkingDirectory = paths.InstallDir })?.Dispose();
            form.Close();
        }

        async Task ReloadSoonAsync()
        {
            await Task.Delay(1200);
            form.ReloadPage();
        }

        void Stop(bool pause)
        {
            if (!pause) sessionTotal = -1;
            if (running == null) return;
            pausing = pause;
            running.Cancel();
        }

        /// <summary>下载线程报来的进度：换算成这一轮的总进度和平滑后的速度再推给页面</summary>
        sealed class Reporter : IProgress<UpdateProgress>
        {
            readonly TowerHost host;
            readonly string kind;
            readonly long already, total;
            readonly CancellationToken cancel;
            readonly Stopwatch clock = Stopwatch.StartNew();
            readonly object gate = new object();
            long lastBytes;
            double lastTime, speed;

            public Reporter(TowerHost host, string kind, long already, long total, CancellationToken cancel)
            {
                this.host = host; this.kind = kind; this.already = already; this.total = total; this.cancel = cancel;
            }

            public void Report(UpdateProgress value)
            {
                if (cancel.IsCancellationRequested) return;
                if (value.Phase == UpdatePhase.Installing)
                {
                    host.installing = true;
                    host.Progress(kind, "installing", 100);
                    return;
                }
                if (value.Phase != UpdatePhase.Downloading) return;
                double rate;
                lock (gate)
                {
                    var now = clock.Elapsed.TotalSeconds;
                    if (value.DoneBytes > lastBytes && now - lastTime >= 0.3)
                    {
                        var instant = (value.DoneBytes - lastBytes) / (now - lastTime);
                        speed = speed == 0 ? instant : speed * 0.6 + instant * 0.4;
                        lastBytes = value.DoneBytes;
                        lastTime = now;
                    }
                    rate = speed;
                }
                var done = already + value.DoneBytes;
                host.Progress(kind, "downloading", total == 0 ? 100 : done * 100.0 / total, total, rate);
            }
        }

        static bool GameRunning()
        {
            var processes = Process.GetProcessesByName("eldenring");
            foreach (var process in processes) process.Dispose();
            return processes.Length > 0;
        }

        // ---------------- 联机大厅 ----------------
        // 房主：在这里开房（房间名、人数、可选房间密码），启动器生成无缝联机密码写进配置，再把房间信息写进模组目录的
        // tower-lobby.json，游戏里的 DLL 用 Steam 大厅公开房间（native/src/lobby.rs）。其他人：列出房间，点加入时
        // 启动器写入这个房间的无缝联机密码再启动游戏。玩家全程不用看、也不用填无缝联机密码。

        SteamLobby? steam;
        List<LobbyRoom> rooms = new List<LobbyRoom>();
        string LobbyFile => Path.Combine(paths.InstallDir, "tower-lobby.json");
        string LobbyVersion()
        {
            var state = InstallState.Load(paths.InstallDir);
            if (state == null) return "dev";
            return state.DisplayVersion.Length > 0 ? state.DisplayVersion : "v" + state.Version;
        }
        void CloseLobby()
        {
            if (steam == null) return;
            try { steam.Dispose(); } catch (Exception e) { Log.Write("关闭 Steam 大厅连接：" + e.Message); }
            steam = null;
        }
        async Task LobbyListAsync()
        {
            var gameDir = GameDir() ?? FindGameDir();
            if (gameDir == null) { form.Post("lobby", "rooms", new { error = "没有找到游戏目录，请先在设置里选择" }); return; }
            try
            {
                steam ??= SteamLobby.Open(gameDir);
                rooms = await steam.ListAsync();
                var version = LobbyVersion();
                form.Post("lobby", "rooms", new
                {
                    version,
                    mine = OwnRoom(),
                    rooms = rooms.Select(r => new
                    {
                        id = r.Id.ToString(), name = r.Name, host = r.Host, players = r.Players, max = r.Max, state = r.State,
                        locked = r.Locked, open = r.Open, started = r.Started, version = r.Version, sameVersion = r.Version == version, full = r.Max > 0 && r.Players >= r.Max,
                    }).ToArray(),
                });
            }
            catch (Exception e) when (e is InvalidOperationException || e is TimeoutException || e is DllNotFoundException || e is EntryPointNotFoundException || e is BadImageFormatException)
            {
                Log.Write("大厅列表失败：" + e.Message);
                CloseLobby();
                form.Post("lobby", "rooms", new { error = e.Message });
            }
        }
        /// <summary>
        /// 自己开的房间（模组目录里的 tower-lobby.json 与 DLL 写的 tower-lobby-status.txt）：Steam 的大厅查询不一定列出自己开的房间，
        /// 这里单独显示房间名与状态（正在创建 / 已上线 / 创建失败及原因）；游戏没在运行时不显示
        /// </summary>
        object? OwnRoom()
        {
            if (!GameRunning() || !File.Exists(LobbyFile)) return null;
            string name;
            try { using var doc = JsonDocument.Parse(File.ReadAllText(LobbyFile)); name = Text(doc.RootElement, "name"); }
            catch (Exception e) when (e is JsonException || e is IOException) { return null; }
            var statusFile = Path.Combine(paths.InstallDir, "tower-lobby-status.txt");
            var line = File.Exists(statusFile) && File.GetLastWriteTimeUtc(statusFile) >= File.GetLastWriteTimeUtc(LobbyFile) ? File.ReadAllText(statusFile).Trim() : "";
            var space = line.IndexOf(' ');
            var state = space > 0 ? line.Substring(0, space) : line;
            var detail = space > 0 ? line.Substring(space + 1) : "";
            return new
            {
                name,
                state = state.Length == 0 ? "missing" : state,
                detail = state == "open"
                    ? (detail == "1" ? "已在 Steam 上线，队友可以加入" : "已在 Steam 上线，但你还没用无缝联机的道具开启联机，队友暂时连不上")
                    : state.Length == 0 ? "游戏里还没有开始创建：确认游戏用的是新版模组，并已进入游戏" : detail,
                ready = state == "open" && detail == "1",
            };
        }
        /// <summary>写入这个房间的无缝联机密码，并切到无缝联机模式</summary>
        bool UseSeamlessPassword(string password)
        {
            if (!(Seamless() is { } seamless) || !seamless.Installed) { Toast("没有找到无缝联机：请先安装无缝联机（设置 → 联机）", "error", 0); return false; }
            seamless.Write(SeamlessSettings.Password, password);
            settings.Mode = HostSettings.Seamless;
            settings.Save(paths.SettingsFile);
            PushStatus();
            return true;
        }
        async Task LobbyHostAsync(JsonElement payload)
        {
            var name = Text(payload, "name").Trim();
            var roomPassword = Text(payload, "password");
            var max = payload.ValueKind == JsonValueKind.Object && payload.TryGetProperty("max", out var m) && m.TryGetInt32(out var n) ? n : 4;
            if (name.Length == 0 || name.Length > 32) { Toast("房间名请填 1–32 个字", "warn"); return; }
            if (max < 2 || max > 8) { Toast("房间人数为 2–8", "warn"); return; }
            if (roomPassword.Length > 32) { Toast("房间密码最多 32 个字", "warn"); return; }
            var locked = roomPassword.Length > 0;
            var salt = locked ? SteamLobby.RandomSalt() : "";
            var seamlessPassword = locked ? SteamLobby.Derive(salt, roomPassword) : SteamLobby.RandomPassword();
            await LaunchAsync(lobbyHost: true, prepare: () =>
            {
                if (!UseSeamlessPassword(seamlessPassword)) return false;
                var room = new Dictionary<string, object> { ["name"] = name, ["max"] = max, ["locked"] = locked, ["salt"] = salt, ["password"] = locked ? "" : seamlessPassword, ["version"] = LobbyVersion(), ["password_sha256"] = LobbyLaunch.Fingerprint(seamlessPassword) };
                File.WriteAllText(LobbyFile, JsonSerializer.Serialize(room), new UTF8Encoding(false));
                var statusFile = Path.Combine(paths.InstallDir, "tower-lobby-status.txt");
                if (File.Exists(statusFile)) File.Delete(statusFile);
                Log.Write($"大厅开房：{name}，{max} 人，{(locked ? "私密" : "公开")}");
                return true;
            });
        }
        async Task LobbyJoinAsync(JsonElement payload)
        {
            var id = Text(payload, "id");
            var room = rooms.FirstOrDefault(r => r.Id.ToString() == id);
            if (room == null) { Toast("房间已经不在了，请刷新列表", "warn"); return; }
            if (room.Version != LobbyVersion()) { Toast($"版本不同（房间 {room.Version}，你是 {LobbyVersion()}），请先更新到同一版本", "warn", 6000); return; }
            if (room.Max > 0 && room.Players >= room.Max) { Toast("房间已满", "warn"); return; }
            // 旅程中途加入会让队伍状态对不上：只能在房主回到大赐福时加入
            if (room.Started) { Toast("这个房间的旅程已经开始，等房主回到大赐福后再加入", "warn", 6000); return; }
            string password;
            if (room.Locked)
            {
                var roomPassword = Text(payload, "password");
                if (roomPassword.Length == 0) { Toast("这是私密房间，请输入房间密码", "warn"); return; }
                if (room.Salt.Length < 16) { Toast("房间数据不完整，请刷新列表", "warn"); return; }
                password = SteamLobby.Derive(room.Salt, roomPassword);
            }
            else password = room.Password;
            if (password.Length < 8) { Toast("房间数据不完整，请刷新列表", "warn"); return; }
            await LaunchAsync(prepare: () =>
            {
                if (!UseSeamlessPassword(password)) return false;
                Log.Write($"大厅加入：{room.Name}（房主 {room.Host}）");
                return true;
            });
        }

        // ---------------- 启动游戏 ----------------

        /// <summary>
        /// 和 Start-Solo.cmd / Start-Seamless.cmd 走同一个 Start-Tower.ps1，只是不开控制台窗口；
        /// 脚本失败时把原因写进 -ErrorFile，这里读出来提示。
        /// </summary>
        bool CanPrepareLaunch()
        {
            var workerRunning = false;
            if (launchWorkerId.HasValue) {
                try {
                    using var worker = Process.GetProcessById(launchWorkerId.Value);
                    workerRunning = !worker.HasExited;
                } catch (ArgumentException) { }
                if (!workerRunning) launchWorkerId = null;
            }
            var reason = LobbyLaunch.BlockReason(GameRunning(), launching, workerRunning, running != null);
            if (reason == null) return true;
            Toast(reason, "warn", 6000);
            return false;
        }

        void PushLaunchProgress()
        {
            var p = launchProgress;
            if (p == null) return;
            form.Post("game", "launchProgress", new { game = Game, phase = p.Phase, text = p.Text, detail = p.Detail,
                done = p.Done, total = p.Total, percent = p.Percent, elapsed = p.Seconds, active = !p.Terminal });
        }

        async Task LaunchAsync(bool lobbyHost = false, Func<bool>? prepare = null)
        {
            if (!CanPrepareLaunch()) return;
            var installedState = InstallState.Load(paths.InstallDir);
            if (latest != null && (installedState == null || latest.Version > installedState.Version)) {
                Toast("请先更新到最新版，再启动游戏", "warn", 6000);
                PushStatus();
                return;
            }
            launching = true;
            launchProgress = new LaunchProgress(); PushLaunchProgress();
            try
            {
            var script = Path.Combine(paths.InstallDir, "Start-Tower.ps1");
            var wrapper = Path.Combine(paths.InstallDir, "Launch-Logged.ps1");
            if (!File.Exists(wrapper)) { Toast("缺少 Launch-Logged.ps1，请完整更新启动器文件", "error", 0); return; }
            if (!File.Exists(script)) { Toast("缺少 Start-Tower.ps1：请在设置 → 更新里「校验并修复文件」", "error", 6000); return; }
            var gameDir = GameDir() ?? FindGameDir() ?? ChooseGameDir();
            if (gameDir == null) return;
                if (prepare != null && !prepare()) return;
                CloseLobby();
                if (!lobbyHost && File.Exists(LobbyFile)) File.Delete(LobbyFile);
                var options = TowerOptions.Read(OptionsFile);
                options.TryGetValue("dlc", out var dlcMode);
                if (string.Equals(dlcMode, "weapons", StringComparison.OrdinalIgnoreCase)) {
                    TowerOptions.Write(OptionsFile, "dlc", "none");
                    dlcMode = "none";
                }
                var dlcWarning = DlcLaunchCheck.Warning(gameDir, settings.Mode == HostSettings.Seamless, dlcMode);
                if (dlcWarning != null)
                {
                    var choice = MessageBox.Show(form, dlcWarning, "DLC 内容启动警告", MessageBoxButtons.OKCancel,
                        MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2);
                    Log.Write("DLC 启动警告：" + (choice == DialogResult.OK ? "用户确认继续" : "用户取消启动"));
                    if (choice != DialogResult.OK) return;
                }
                var errorFile = paths.LaunchErrorFile;
                if (File.Exists(errorFile)) File.Delete(errorFile);
                var mode = settings.Mode;
                var logs = Path.Combine(paths.LaunchLogs, DateTime.Now.ToString("yyyyMMdd-HHmmss") + "-" + Guid.NewGuid().ToString("N").Substring(0, 8));
                Directory.CreateDirectory(logs);
                var arguments = $"-NoProfile -NonInteractive -ExecutionPolicy Bypass -File {Quote(wrapper)} -Entry {Quote(script)} -Mode {mode} -Loader {settings.Loader} -GameDir {Quote(gameDir)} -ErrorFile {Quote(errorFile)} -LogDirectory {Quote(logs)}";
                Log.Write("开始游戏：" + arguments);
                var output = new LaunchLogReader(Path.Combine(logs, "stdout.log"));
                var nativeLog = new LaunchLogReader(Path.Combine(paths.InstallDir,
                    mode == HostSettings.Seamless ? "seamless-entry/runtime/tower-preview.log" : "tower-preview.log"), true);
                var powershell = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "WindowsPowerShell", "v1.0", "powershell.exe");
                using var process = Process.Start(new ProcessStartInfo(powershell, arguments) { UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = paths.InstallDir })!;
                launchWorkerId = process.Id;
                Toast("正在启动黄金树爬塔…", "info", 6000);
                var localAssets = UsesLocalAssets();
                launchProgress.Set(localAssets ? "checking" : "starting", localAssets ? "正在检查本地资源" : "正在启动加载器",
                    localAssets ? "首次生成或修复可能需要数分钟；进度会实时更新" : settings.Loader);
                PushLaunchProgress();
                var launchWaitTicks = localAssets ? 3600 : 120;
                if (localAssets) Toast("正在检查本地资源；首次生成或修复可能需要数分钟，请勿重复启动", "info", 12000);
                if (mode == HostSettings.Seamless && Seamless() is { } seamless && seamless.Installed && !seamless.Version().Verified)
                    Toast("这个无缝联机版本还没验证过：照常启动，联机复活修复会自动关闭；联机双方要用同一个版本", "warn", 8000);
                // 加载器拉起游戏后脚本可能一直等到游戏退出，也可能马上结束：两种情况都只盯「脚本报错」和「游戏起来了」
                var started = false;
                var startedTick = -1;
                for (var waited = 0; !process.HasExited || (process.ExitCode == 0 && (!started && waited < launchWaitTicks || started && !launchProgress.Terminal && waited - startedTick < 120)); waited++)
                {
                    await Task.Delay(500);
                    foreach (var line in output.Read()) launchProgress.Output(line);
                    foreach (var line in nativeLog.Read()) launchProgress.Native(line);
                    if (started && !launchProgress.Terminal && !GameRunning()) {
                        launchProgress.End("failed", "游戏在画面就绪前退出", "请打开启动日志查看原因"); PushLaunchProgress(); break;
                    }
                    if (started && !launchProgress.Terminal && waited - startedTick >= 120)
                        launchProgress.End("running", "游戏进程已启动", "尚未收到首帧信号，请查看游戏窗口；无需重复启动");
                    PushLaunchProgress();
                    if (started && launchProgress.Terminal && settings.CloseExit) { form.Close(); return; }
                    if (!started && waited >= launchWaitTicks - 1) {
                        Log.Write("等待游戏进程超时；后台日志：" + logs);
                        launchProgress.End("failed", "启动等待超时", "后台可能仍在准备资源，请打开日志查看详情"); PushLaunchProgress();
                        Toast(localAssets ? "本地资源准备或启动耗时过长，请查看后台日志" : "60 秒内未检测到游戏启动，请查看后台日志", "error", 0, "打开日志", "openLogs");
                        return;
                    }
                    if (started || !GameRunning()) continue;
                    started = true;
                    startedTick = waited; launchProgress.GameDetected(); PushLaunchProgress();
                    Log.Write("游戏已启动");
                }
                if (process.HasExited && process.ExitCode != 0)
                {
                    var reason = LaunchResult.Summary(errorFile, logs, process.ExitCode);
                    Log.Write($"启动脚本退出码 {process.ExitCode}：{reason}");
                    launchProgress.End("failed", "启动失败", reason); PushLaunchProgress();
                    Toast("启动失败：" + reason, "error", 0, "打开日志", "openLogs");
                }
            }
            catch (Exception e) when (e is IOException || e is Win32Exception || e is UnauthorizedAccessException)
            {
                Log.Write("启动失败：" + e);
                launchProgress.End("failed", "启动失败", e.Message); PushLaunchProgress();
                Toast("启动失败：" + e.Message, "error", 0, "打开日志", "openLogs");
            }
            finally {
                launching = false;
                if (!launchProgress.Terminal) launchProgress.End("cancelled", "启动已结束", "可以查看日志或重新启动");
                PushLaunchProgress();
            }
        }

        bool UsesLocalAssets()
        {
            var mode = Path.Combine(paths.InstallDir, "asset-mode.txt");
            return File.Exists(mode) && File.ReadAllText(mode).Trim() == "local-v1";
        }

        bool LocalAssetPreparationBusy()
        {
            var path = Path.Combine(paths.InstallDir, "asset-patches", ".restore.lock");
            if (!File.Exists(path)) return false;
            try { using var guard = File.Open(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite); return false; }
            catch (IOException) { return true; }
        }

        static bool HasGame(string dir) => File.Exists(Path.Combine(dir, "eldenring.exe"));

        /// <summary>GamePath.txt 里记住的 Game 目录（和 Start-Tower.ps1 共用），失效了返回 null</summary>
        string? GameDir()
        {
            try
            {
                if (!File.Exists(paths.GamePathFile)) return null;
                var dir = File.ReadAllText(paths.GamePathFile).Trim().Trim('"');
                return dir.Length > 0 && HasGame(dir) ? dir : null;
            }
            catch (IOException) { return null; }
            catch (ArgumentException) { return null; }
        }

        void SaveGameDir(string dir)
        {
            File.WriteAllText(paths.GamePathFile, dir, new UTF8Encoding(true));
            PushValues(InstallState.Load(paths.InstallDir));
        }

        /// <summary>在 Steam 的各个库目录里找艾尔登法环，找到就记下来</summary>
        string? FindGameDir()
        {
            if (!(Registry.GetValue(@"HKEY_CURRENT_USER\Software\Valve\Steam", "SteamPath", null) is string steam) || steam.Length == 0) return null;
            var libraries = new List<string> { steam };
            try
            {
                var folders = Path.Combine(steam, "steamapps", "libraryfolders.vdf");
                if (File.Exists(folders))
                    foreach (Match match in Regex.Matches(File.ReadAllText(folders), "\"path\"\\s+\"([^\"]+)\""))
                        libraries.Add(match.Groups[1].Value.Replace(@"\\", @"\"));
            }
            catch (IOException) { }
            foreach (var library in libraries)
            {
                try
                {
                    var dir = Path.GetFullPath(Path.Combine(library, "steamapps", "common", "ELDEN RING", "Game"));
                    if (!HasGame(dir)) continue;
                    SaveGameDir(dir);
                    Toast("已找到游戏目录：" + dir, "success", 5000);
                    return dir;
                }
                catch (ArgumentException) { }
            }
            return null;
        }

        string? ChooseGameDir()
        {
            using var dialog = new FolderBrowserDialog { Description = "选择艾尔登法环的 Game 文件夹（里面有 eldenring.exe）", ShowNewFolderButton = false };
            var current = GameDir();
            if (current != null) dialog.SelectedPath = current;
            while (dialog.ShowDialog(form) == DialogResult.OK)
            {
                var dir = dialog.SelectedPath;
                if (!HasGame(dir) && HasGame(Path.Combine(dir, "Game"))) dir = Path.Combine(dir, "Game");
                if (HasGame(dir)) { SaveGameDir(dir); return dir; }
                MessageBox.Show(form, "这个文件夹里没有 eldenring.exe。\n请选择游戏安装目录里的 Game 文件夹，例如：\n…\\steamapps\\common\\ELDEN RING\\Game",
                    MainForm.Title, MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
            return null;
        }

        // ---------------- 本地功能 ----------------

        void OpenWiki()
        {
            var path = Path.Combine(paths.InstallDir, "launcher", "ui", "wiki", "index.html");
            if (!File.Exists(path)) { Toast("百科文件缺失，请校验并修复启动器文件", "warn"); return; }
            try { Process.Start(new ProcessStartInfo(path) { UseShellExecute = true })?.Dispose(); }
            catch (Exception e) { Toast("打开百科失败：" + e.Message, "error"); }
        }

        public static void OpenUrl(string url)
        {
            if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || (uri.Scheme != Uri.UriSchemeHttps && uri.Scheme != Uri.UriSchemeHttp)) return;
            try { Process.Start(new ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true })?.Dispose(); }
            catch (Win32Exception e) { Log.Write("打开网页失败：" + e.Message); }
        }

        static void OpenFolder(string dir) => Process.Start("explorer.exe", Quote(dir))?.Dispose();

        void OpenLogs()
        {
            Directory.CreateDirectory(paths.LaunchLogs);
            var latestLog = Directory.GetDirectories(paths.LaunchLogs).OrderByDescending(Path.GetFileName).FirstOrDefault();
            OpenFolder(latestLog ?? paths.DataDir);
        }

        void OpenModLogs()
        {
            var file=ModLogs.Latest(paths.InstallDir);
            if(file!=null)Process.Start("explorer.exe","/select,"+Quote(file))?.Dispose();
            else {OpenFolder(paths.InstallDir);Toast("尚未生成 tower-preview.log，已打开 Mod 目录；启动游戏后会生成日志", "info", 6000);}
        }

        List<HistoryEntry> SavedHistory()
        {
            var rows=new List<HistoryEntry>();
            foreach(var file in new[]{Path.Combine(paths.UiDir,"release-history.json"),Path.Combine(paths.DataDir,"release-history.json")})
            {
                try {if(File.Exists(file))rows=ReleaseHistory.Merge(rows,ReleaseHistory.ParseGitHub(File.ReadAllText(file)));}
                catch(Exception e) when(e is IOException||e is UnauthorizedAccessException||e is JsonException||e is InvalidOperationException){Log.Write("读取更新历史失败："+e.Message);}
            }
            return rows;
        }

        void PushHistory(List<HistoryEntry> rows,string status,bool loading)
        {
            form.Post("app","releaseHistory",new {status,loading,items=rows.Select(r=>new {tag=r.Tag,title=r.Title,notes=r.Notes,date=r.PublishedAt,url=Repo+"/releases/tag/"+Uri.EscapeDataString(r.Tag)})});
        }

        async Task HistoryAsync()
        {
            if(historyLoading){PushHistory(SavedHistory(),"正在刷新历史记录…",true);return;}
            historyLoading=true;
            var rows=SavedHistory();
            PushHistory(rows,"正在刷新历史记录…",true);
            try
            {
                using var cancel=new CancellationTokenSource(TimeSpan.FromSeconds(15));
                using var response=await http.GetAsync("https://api.github.com/repos/tamikip/erdtree-tower/releases?per_page=100",cancel.Token);
                response.EnsureSuccessStatusCode();
                var json=await response.Content.ReadAsStringAsync();
                rows=ReleaseHistory.Merge(rows,ReleaseHistory.ParseGitHub(json));
                try {File.WriteAllText(Path.Combine(paths.DataDir,"release-history.json"),json,new UTF8Encoding(false));}
                catch(Exception e) when(e is IOException||e is UnauthorizedAccessException){Log.Write("保存更新历史缓存失败："+e.Message);}
                PushHistory(rows,"已刷新 · 最近 100 条发布及随包历史",false);
            }
            catch(Exception e) when(e is HttpRequestException||e is OperationCanceledException||e is JsonException||e is InvalidOperationException)
            {
                Log.Write("刷新更新历史失败："+e.Message);
                PushHistory(rows,rows.Count>0?"在线刷新失败，正在显示本地历史记录":"暂时无法获取记录，请稍后重试或查看发布页",false);
            }
            finally {historyLoading=false;}
        }

        string PackageDocument(string name, string legacy)
        {
            var document = Path.Combine(paths.InstallDir, "docs", name);
            return File.Exists(document) ? document : Path.Combine(paths.InstallDir, legacy);
        }

        static void OpenDocument(string path, string fallbackUrl)
        {
            if (File.Exists(path)) Process.Start("notepad.exe", Quote(path))?.Dispose();
            else OpenUrl(fallbackUrl);
        }

        /// <summary>按 Windows 命令行规则给一个参数加引号（引号前的反斜杠要成对）</summary>
        static string Quote(string argument)
        {
            var quoted = new StringBuilder("\"");
            var slashes = 0;
            foreach (var c in argument)
            {
                if (c == '\\') { slashes++; continue; }
                quoted.Append('\\', c == '"' ? slashes * 2 + 1 : slashes).Append(c);
                slashes = 0;
            }
            return quoted.Append('\\', slashes * 2).Append('"').ToString();
        }

        static string Describe(Exception e) => e switch
        {
            TaskCanceledException _ => "网络超时，请稍后重试",
            HttpRequestException _ => "网络连接失败：" + (e.InnerException?.Message ?? e.Message),
            UnauthorizedAccessException _ => "没有写入权限（模组不要放在 Program Files 这类需要管理员权限的目录）：" + e.Message,
            _ => e.Message,
        };

        static string Size(long bytes)
        {
            if (bytes < 1024 * 1024) return $"{(bytes + 1023) / 1024} KB";
            if (bytes < 1024L * 1024 * 1024) return $"{bytes / 1024.0 / 1024:0.0} MB";
            return $"{bytes / 1024.0 / 1024 / 1024:0.00} GB";
        }
    }
}
