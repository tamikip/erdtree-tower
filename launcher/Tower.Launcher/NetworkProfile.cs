using System;
using System.IO;

namespace Tower.Launcher
{
    static class NetworkProfile
    {
        public const string Steam = "Steam", OnlineFix = "OnlineFix";
        public const string SteamLabel = "正版无缝", OnlineFixLabel = "学习版无缝（Spacewar）";
        public static string Normalize(string? value) => value == OnlineFix ? OnlineFix : Steam;
        public static string Label(string value) => value == OnlineFix ? OnlineFixLabel : SteamLabel;
        public static string AppId(string value) => value == OnlineFix ? "480" : "1245620";
        public static string PathFile(string value) => value == OnlineFix ? "GamePath.OnlineFix.txt" : "GamePath.txt";
        public static string Runtime(string value) => value == OnlineFix ? "onlinefix-runtime" : "runtime";

        public static void AssertDifferent(string selected, string? other)
        {
            if (string.IsNullOrWhiteSpace(other)) return;
            string Clean(string p) => Path.GetFullPath(p.Trim().Trim('"')).TrimEnd('\\', '/');
            if (string.Equals(Clean(selected), Clean(other!), StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("两种模式必须选择不同的游戏目录，避免混用联机补丁。");
        }
    }
}
