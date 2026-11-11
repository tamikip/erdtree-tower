using System;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows.Forms;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;

namespace Tower.Launcher
{
    /// <summary>
    /// 无边框 1280×720 窗口 + WebView2。界面是 launcher/ui 里的静态页面，通过虚拟主机名 https://tower.launcher/ 加载；
    /// 页面发来的 {type,action,payload} 交给 TowerHost，宿主推给页面的消息走 Post。
    /// </summary>
    sealed class MainForm : Form
    {
        public const string Title = "黄金树爬塔启动器";
        const string PageHost = "tower.launcher";
        static readonly Color Backdrop = Color.FromArgb(16, 16, 18);

        readonly LauncherPaths paths;
        WebView2 web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Backdrop };
        readonly TowerHost host;

        public MainForm(LauncherPaths paths)
        {
            this.paths = paths;
            Text = Title;
            FormBorderStyle = FormBorderStyle.None;
            StartPosition = FormStartPosition.CenterScreen;
            AutoScaleDimensions = new SizeF(96F, 96F);
            AutoScaleMode = AutoScaleMode.Dpi;
            ClientSize = new Size(1280, 720);
            BackColor = Backdrop;
            Icon = Icon.ExtractAssociatedIcon(Application.ExecutablePath);
            Controls.Add(web);
            host = new TowerHost(paths, this);
        }

        /// <summary>无边框窗口也要能从任务栏最小化、带系统阴影</summary>
        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
                cp.Style |= 0x00020000; // WS_MINIMIZEBOX
                cp.ClassStyle |= 0x00020000; // CS_DROPSHADOW
                return cp;
            }
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            // Windows 11 圆角（Windows 10 上调用无效果，忽略返回值）
            var round = 2; // DWMWCP_ROUND
            DwmSetWindowAttribute(Handle, 33 /* DWMWA_WINDOW_CORNER_PREFERENCE */, ref round, sizeof(int));
        }

        protected override async void OnLoad(EventArgs e)
        {
            base.OnLoad(e);
            try { await InitAsync(); }
            catch (Exception error)
            {
                Log.Write("界面初始化失败：" + error);
                MessageBox.Show(this, "启动器界面加载失败：" + error.Message, Title, MessageBoxButtons.OK, MessageBoxIcon.Error);
                Close();
            }
        }

        async Task InitAsync()
        {
            // 发布包里 WebView2Loader.dll 和其他依赖一起放在 launcher/；开发时在 runtimes/ 下，SDK 自己能找到
            if (File.Exists(Path.Combine(paths.LibDir, "WebView2Loader.dll"))) CoreWebView2Environment.SetLoaderDllFolderPath(paths.LibDir);
            if (!WebViewInstalled())
            {
                var answer = MessageBox.Show(this, "启动器界面需要 Microsoft Edge WebView2 运行库（Windows 11 自带，Windows 10 大多已装）。\n\n是否打开微软官网下载？装好后重新打开启动器即可。",
                    Title, MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
                if (answer == DialogResult.Yes) TowerHost.OpenUrl("https://go.microsoft.com/fwlink/p/?LinkId=2124703");
                Close();
                return;
            }
            if (!File.Exists(Path.Combine(paths.UiDir, "index.html")))
            {
                MessageBox.Show(this, "启动器界面文件缺失（launcher\\ui）。请重新下载完整包，或用 Start-Tower.cmd 启动。", Title, MessageBoxButtons.OK, MessageBoxIcon.Error);
                Close();
                return;
            }
            try
            {
                var environment = await CoreWebView2Environment.CreateAsync(null, paths.WebViewData);
                await web.EnsureCoreWebView2Async(environment);
            }
            catch (Exception error) when (error.HResult == unchecked((int)0x800700AA))
            {
                Log.Write("WebView2 数据目录占用，尝试独立缓存：" + error);
                Controls.Remove(web);
                web.Dispose();
                web = new WebView2 { Dock = DockStyle.Fill, DefaultBackgroundColor = Backdrop };
                Controls.Add(web);
                var cache = Path.Combine(paths.DataDir, "WebView2-recovery-" + Guid.NewGuid().ToString("N"));
                var environment = await CoreWebView2Environment.CreateAsync(null, cache);
                await web.EnsureCoreWebView2Async(environment);
            }
            var core = web.CoreWebView2;
            var debug = File.Exists(Path.Combine(paths.InstallDir, "tower-debug.enable"));
            core.Settings.AreDevToolsEnabled = debug;
            core.Settings.AreBrowserAcceleratorKeysEnabled = debug;
            core.Settings.AreDefaultContextMenusEnabled = false;
            core.Settings.IsStatusBarEnabled = false;
            core.Settings.IsZoomControlEnabled = false;
            core.Settings.IsPinchZoomEnabled = false;
            core.SetVirtualHostNameToFolderMapping(PageHost, paths.UiDir, CoreWebView2HostResourceAccessKind.DenyCors);
            // 页面里点到的外部链接一律交给系统浏览器，窗口本身只显示自己的界面
            core.NavigationStarting += (s, a) =>
            {
                if (IsOwnPage(a.Uri)) return;
                a.Cancel = true;
                TowerHost.OpenUrl(a.Uri);
            };
            core.NewWindowRequested += (s, a) => { a.Handled = true; TowerHost.OpenUrl(a.Uri); };
            // 处理放到事件之外：指令里可能弹对话框，WebView2 的事件回调里不宜开模态循环
            core.WebMessageReceived += (s, a) =>
            {
                var json = a.WebMessageAsJson;
                BeginInvoke(new Action(() => host.Handle(json)));
            };
            core.NavigationCompleted += (s, a) => { if (a.IsSuccess) host.PageReady(); };
            core.ProcessFailed += (s, a) =>
            {
                Log.Write($"WebView2 进程异常：{a.ProcessFailedKind}");
                if (a.ProcessFailedKind == CoreWebView2ProcessFailedKind.RenderProcessExited || a.ProcessFailedKind == CoreWebView2ProcessFailedKind.RenderProcessUnresponsive) core.Reload();
            };
            ReloadPage();
        }

        static bool WebViewInstalled()
        {
            try { return !string.IsNullOrEmpty(CoreWebView2Environment.GetAvailableBrowserVersionString()); }
            catch (WebView2RuntimeNotFoundException) { return false; }
        }

        static bool IsOwnPage(string uri) =>
            Uri.TryCreate(uri, UriKind.Absolute, out var parsed) && parsed.Scheme == Uri.UriSchemeHttps && parsed.Host.Equals(PageHost, StringComparison.OrdinalIgnoreCase);

        /// <summary>宿主 → 页面。任何线程都可以调，回到界面线程再发。</summary>
        public void Post(string type, string action, object? payload)
        {
            var json = JsonSerializer.Serialize(new { type, action, payload });
            if (!InvokeRequired) { Send(json); return; }
            // 窗口已经关掉时后台任务的进度直接丢弃
            try { if (IsHandleCreated && !IsDisposed) BeginInvoke(new Action(() => Send(json))); }
            catch (InvalidOperationException) { }
        }

        void Send(string json)
        {
            if (IsDisposed || web.CoreWebView2 == null) return;
            web.CoreWebView2.PostWebMessageAsJson(json);
        }

        /// <summary>启动器自己的界面文件更新后重新加载页面（状态由 PageReady 重新推送）</summary>
        public void ReloadPage() => web.CoreWebView2?.Navigate($"https://{PageHost}/index.html?ui={File.GetLastWriteTimeUtc(Path.Combine(paths.UiDir, "index.html")).Ticks}");

        /// <summary>页面在可拖动区域按下鼠标：交给系统按标题栏拖动处理。消息到达时鼠标已经松开就不拖，免得窗口粘在指针上。</summary>
        public void StartDrag()
        {
            if (MouseButtons != MouseButtons.Left) return;
            ReleaseCapture();
            SendMessage(Handle, 0xA1 /* WM_NCLBUTTONDOWN */, (IntPtr)2 /* HTCAPTION */, IntPtr.Zero);
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            // 正在替换文件时不让关：虽然强杀后下次启动也能还原，但没必要让玩家碰上
            if (host.Installing && e.CloseReason == CloseReason.UserClosing)
            {
                e.Cancel = true;
                host.Toast("正在安装更新，马上就好", "warn");
                return;
            }
            host.Dispose();
            base.OnFormClosing(e);
        }

        [DllImport("user32.dll")] static extern bool ReleaseCapture();
        [DllImport("user32.dll")] static extern IntPtr SendMessage(IntPtr window, int message, IntPtr wParam, IntPtr lParam);
        [DllImport("dwmapi.dll")] static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);
    }
}
