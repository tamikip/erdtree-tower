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
            if (!seamless && noDlc) return null;
            hasArchive = hasArchive ?? HasArchive;
            var present = hasArchive(Path.Combine(gameDir, "DLC.bhd")) && hasArchive(Path.Combine(gameDir, "DLC.bdt"));
            if (present && !noDlc) return null;
            var reason = present
                ? "你在设置中选择了没有 DLC，但无缝联机模式仍会启用 DLC 内容。"
                : "游戏目录中的黄金树之影 DLC 资源文件（DLC.bhd / DLC.bdt）缺失、为空或无法读取。";
            return reason + "\n\n当前启动模式会启用 DLC 敌人与物品，缺少所需资源可能导致加载失败或闪退。"
                + "\n本检查仅查看本地文件，不能验证账号是否拥有 DLC，也不能验证资源是否完整。"
                + "\n\n确定：仍然启动。\n取消：返回启动器，核对游戏及 DLC 安装。";
        }

        static bool HasArchive(string path)
        {
            try { return new FileInfo(path).Length > 0; }
            catch (IOException) { return false; }
            catch (UnauthorizedAccessException) { return false; }
        }
    }
}
