using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;

namespace Tower.Launcher
{
    /// <summary>
    /// 模组目录的 tower-options.ini：游戏里的 DLL 启动时读（格式见 src/options.rs），所以用 .cmd 启动也生效。
    /// 启动器只改自己认识的行，注释和其他行原样保留；这是玩家自己的文件，不在发布清单里，更新不会覆盖。
    /// </summary>
    static class TowerOptions
    {
        public const string FileName = "tower-options.ini";

        /// <summary>可改键的功能：设置页的 key.&lt;id&gt; 与文件里的 key.&lt;id&gt;，以及原本的键</summary>
        public static readonly (string Id, string Label, string Default)[] Hotkeys =
        {
            ("settle", "快速结算", "Home"),
            ("overlay", "显示/隐藏爬塔界面", "Insert"),
            ("archive", "旅程档案", "J"),
            ("build", "构筑面板", "C"),
            ("debug", "调试菜单", "Delete"),
            ("skip", "跳过本关", "End"),
        };

        /// <summary>DLC 模式：设置页显示的文字 ↔ 文件里的 dlc= 值（src/dlc.rs 的 Mode）</summary>
        public static readonly (string Id, string Label)[] DlcModes =
        {
            ("full", "有 DLC"),
            ("none", "没有 DLC"),
            ("weapons", "没有 DLC，保留 DLC 武器（实验）"),
        };

        /// <summary>界面按键提示：设置页显示的文字 ↔ 文件里的 prompts= 值（src/options.rs 的 Prompts）</summary>
        public static readonly (string Id, string Label)[] PromptModes =
        {
            ("keyboard", "键鼠"),
            ("pad", "手柄"),
        };

        public static readonly (string Id, string Label)[] Languages =
        {
            ("auto", "跟随游戏 / Follow game"),
            ("zh-CN", "简体中文 / Chinese"),
            ("en-US", "English"),
        };
        public static string LanguageLabel(Dictionary<string, string> values) =>
            Languages.FirstOrDefault(m => values.TryGetValue("language", out var v) && m.Id.Equals(v, StringComparison.OrdinalIgnoreCase)).Label ?? Languages[0].Label;

        public static string PromptLabel(Dictionary<string, string> values) =>
            PromptModes.FirstOrDefault(m => values.TryGetValue("prompts", out var v) && m.Id.Equals(v, StringComparison.OrdinalIgnoreCase)).Label ?? PromptModes[0].Label;

        public static string DlcLabel(Dictionary<string, string> values) =>
            DlcModes.FirstOrDefault(m => values.TryGetValue("dlc", out var v) && m.Id.Equals(v, StringComparison.OrdinalIgnoreCase)).Label ?? DlcModes[0].Label;

        static readonly string Header = "# 黄金树爬塔启动参数：启动器会改写这个文件，也可以手动改，下次启动游戏生效。说明见 src/options.rs";

        /// <summary>名字一律小写；不存在或读不了返回空表</summary>
        public static Dictionary<string, string> Read(string file)
        {
            var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            try
            {
                if (!File.Exists(file)) return values;
                foreach (var raw in File.ReadAllLines(file))
                {
                    var line = raw.Trim();
                    if (line.Length == 0 || line[0] == '#' || line[0] == ';') continue;
                    var at = line.IndexOf('=');
                    if (at > 0) values[line.Substring(0, at).Trim().ToLowerInvariant()] = line.Substring(at + 1).Trim();
                }
            }
            catch (IOException e) { Log.Write("读取 " + FileName + " 失败：" + e.Message); }
            catch (UnauthorizedAccessException e) { Log.Write("读取 " + FileName + " 失败：" + e.Message); }
            return values;
        }

        public static string Key(Dictionary<string, string> values, string id, string fallback) =>
            values.TryGetValue("key." + id, out var key) && key.Length > 0 ? key : fallback;

        public static bool Debug(Dictionary<string, string> values) =>
            values.TryGetValue("debug", out var v) && new[] { "1", "true", "on", "yes" }.Contains(v.ToLowerInvariant());

        /// <summary>改一行（没有就追加），其余内容不动</summary>
        public static void Write(string file, string name, string value)
        {
            var lines = File.Exists(file) ? File.ReadAllLines(file).ToList() : new List<string> { Header };
            var index = lines.FindIndex(l =>
            {
                var t = l.Trim();
                var at = t.IndexOf('=');
                return at > 0 && t[0] != '#' && t[0] != ';' && t.Substring(0, at).Trim().Equals(name, StringComparison.OrdinalIgnoreCase);
            });
            if (index >= 0) lines[index] = $"{name}={value}";
            else lines.Add($"{name}={value}");
            File.WriteAllText(file, string.Join("\r\n", lines) + "\r\n", new UTF8Encoding(false));
        }
    }
}
