using System;

namespace Tower.Assets
{
    internal static class Progress
    {
        public static void Report(string phase, int done, int total, string text, string detail = "")
        {
            Console.WriteLine("TOWER_PROGRESS|" + phase + "|" + done + "|" + total + "|" + text + "|" + detail.Replace('|', '/').Replace('\r', ' ').Replace('\n', ' '));
        }
        public static string Name(string path) => path.StartsWith("locales/") ? "英文界面贴图" : path.StartsWith("sfx/") ? "游戏特效资源" : "界面与物品图集";
    }
}
