using System;
using System.Security.Cryptography;
using System.Text;

namespace Tower.Launcher
{
    internal static class LobbyLaunch
    {
        public static string? PasswordError(bool locked, string password, string fingerprint)
        {
            if (!locked && fingerprint.Length == 0) return null;
            if (fingerprint.Length != 64 || !System.Linq.Enumerable.All(fingerprint, Uri.IsHexDigit))
                return "房间缺少密码校验信息，请房主更新模组并重新开房后刷新列表";
            if (!string.Equals(Fingerprint(password), fingerprint, StringComparison.OrdinalIgnoreCase))
                return locked ? "房间密码错误，请重新输入" : "房间密码信息不一致，请刷新列表或让房主重新开房";
            return null;
        }
        public static string? BlockReason(bool game, bool launching, bool worker, bool updating)
        {
            if (game) return "游戏正在运行，联机密码已锁定；请保存退出后再开房、加入或改密码";
            if (launching || worker) return "游戏正在启动，暂时不能切换房间或修改联机密码，请稍候";
            if (updating) return "正在更新，完成后再开始游戏或修改联机密码";
            return null;
        }

        public static string Fingerprint(string password)
        {
            using var hash = SHA256.Create();
            return BitConverter.ToString(hash.ComputeHash(Encoding.UTF8.GetBytes(password))).Replace("-", "").ToLowerInvariant();
        }
    }
}
