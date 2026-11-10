using System;
using System.IO;

namespace Tower.Launcher
{
    static class DlcLaunchCheck
    {
        public static string? Warning(string gameDir, bool seamless, string? mode, Func<string, bool>? hasArchive = null)
        {
            var noDlc = string.Equals(mode, "none", StringComparison.OrdinalIgnoreCase)
                || string.Equals(mode, "weapons", StringComparison.OrdinalIgnoreCase);
            // 选了没有 DLC：单人与联机都按没有 DLC 运行（联机时队伍里有人没有 DLC，整队都不出 DLC 内容），不需要提示
            if (noDlc) return null;
            hasArchive = hasArchive ?? HasArchive;
            var present = hasArchive(Path.Combine(gameDir, "DLC.bhd")) && hasArchive(Path.Combine(gameDir, "DLC.bdt"));
            if (present) return null;
            return "游戏目录中的黄金树之影 DLC 资源文件（DLC.bhd / DLC.bdt）缺失、为空或无法读取。"
                + "\n\n当前设置为「有 DLC」，会启用 DLC 敌人与物品，缺少所需资源可能导致加载失败或闪退。"
                + "\n没有购买 DLC 的话，请在设置 → 游戏里把 DLC 改为「没有 DLC」；联机时只要队伍里有人没有 DLC，整队都会按没有 DLC 运行。"
                + "\n本检查仅查看本地文件，不能验证账号是否拥有 DLC，也不能验证资源是否完整。"
                + "\n\n确定：仍然启动。\n取消：返回启动器。";
        }

        static bool HasArchive(string path)
        {
            try { return new FileInfo(path).Length > 0; }
            catch (IOException) { return false; }
            catch (UnauthorizedAccessException) { return false; }
        }
    }
}
