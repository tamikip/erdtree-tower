using System;
using System.IO;
using System.Text.Json;

namespace Tower.Launcher
{
    /// <summary>宿主要用到的设置（启动模式、下载线路、自动检查更新、启动后关闭）。界面上的值由宿主推送，以这里为准。</summary>
    sealed class HostSettings
    {
        public const string Solo = "Solo", Seamless = "Seamless";
        public const string Me3 = "ME3", Yafsml = "YAFSML";
        public const string MirrorFirst = "mirror", GitHubFirst = "github";

        public string Mode { get => Seamless; set { } }
        public string Loader { get; set; } = Me3;
        /// <summary>优先走哪条下载线路（另一条兜底）</summary>
        public string Route { get; set; } = MirrorFirst;
        public bool AutoCheck { get; set; } = true;
        public bool CloseExit { get; set; }

        public static HostSettings Load(string path)
        {
            try
            {
                if (File.Exists(path))
                {
                    var settings = JsonSerializer.Deserialize<HostSettings>(File.ReadAllText(path)) ?? new HostSettings();
                    if (settings.Loader != Yafsml) settings.Loader = Me3;
                    if (settings.Route != GitHubFirst) settings.Route = MirrorFirst;
                    return settings;
                }
            }
            catch (JsonException) { }
            catch (IOException) { }
            return new HostSettings();
        }

        public void Save(string path)
        {
            try { File.WriteAllText(path, JsonSerializer.Serialize(this)); }
            catch (IOException e) { Log.Write("保存设置失败：" + e.Message); }
            catch (UnauthorizedAccessException e) { Log.Write("保存设置失败：" + e.Message); }
        }
    }
}
