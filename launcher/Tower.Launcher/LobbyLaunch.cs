using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text;

namespace Tower.Launcher
{
    sealed class LobbyRoom
    {
        public ulong Id;
        public string Name = "", Host = "", State = "", Version = "", Salt = "", Password = "", PasswordFingerprint = "", Code = "";
        public int Players, Max;
        public bool Locked, Open, Started;
    }
    internal static class LobbyLaunch
    {
        public static string NewRoomCode()
        {
            var bytes = new byte[8];
            using (var rng = RandomNumberGenerator.Create()) rng.GetBytes(bytes);
            return "T" + BitConverter.ToString(bytes).Replace("-", "");
        }
        public static string? NormalizeRoomCode(string text)
        {
            var code = (text ?? "").Trim().Replace("-", "").ToUpperInvariant();
            return code.Length == 17 && code[0] == 'T' && code.Skip(1).All(Uri.IsHexDigit) ? code : null;
        }
        public static string? JoinError(LobbyRoom room, string version)
        {
            if (room.Version != version) return "房间版本不同，请与房主更新到同一版本";
            if (room.Max < 2 || room.Max > 6 || room.Players < 1) return "房间人数信息无效，请刷新列表";
            if (room.Players >= room.Max) return "房间已满，请选择其他队伍";
            if (room.Started) return "旅程已经开始，等房主回到大赐福后再加入";
            if (!room.Open) return "房主尚未开放联机，请稍后再试";
            return null;
        }
        public static LobbyRoom? QuickCandidate(IEnumerable<LobbyRoom> rooms, string version)
            => rooms.Where(r => !r.Locked && JoinError(r, version) == null)
                .OrderByDescending(r => r.Players).ThenBy(r => r.Id).FirstOrDefault();
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
            if (game) return "游戏正在运行，请保存退出后再开房或切换队伍";
            if (launching || worker) return "游戏正在启动，暂时不能切换队伍，请稍候";
            if (updating) return "正在更新，完成后再开始游戏或加入队伍";
            return null;
        }

        public static string Fingerprint(string password)
        {
            using var hash = SHA256.Create();
            return BitConverter.ToString(hash.ComputeHash(Encoding.UTF8.GetBytes(password))).Replace("-", "").ToLowerInvariant();
        }
    }
}
