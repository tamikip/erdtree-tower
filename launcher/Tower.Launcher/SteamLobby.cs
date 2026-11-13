using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;

namespace Tower.Launcher
{
    /// <summary>大厅里的一个房间（Steam 大厅数据，键见 src/lobby.rs 的 HostRoom::data）</summary>
    sealed class LobbyRoom
    {
        public ulong Id;
        public string Name = "", Host = "", State = "", Version = "", Salt = "", Password = "", PasswordFingerprint = "";
        public int Players, Max;
        public bool Locked, Open, Started;
    }

    /// <summary>
    /// 联机大厅（浏览端）：用游戏目录里的 steam_api64 以艾尔登法环的应用编号临时登录 Steam，列出带爬塔标记的大厅；
    /// 只在浏览时开着，启动游戏前关掉（Steam 上会短暂显示正在玩艾尔登法环）。
    /// 房主侧的大厅由游戏里的 DLL 创建（native/src/lobby.rs）。CallResult 不注册回调，按调用编号轮询。
    /// </summary>
    sealed class SteamLobby : IDisposable
    {
        public const string AppId = "1245620";
        const string Tag = "erdtree-tower";
        const int MaxRooms = 500;
        /// <summary>LobbyMatchList_t：k_iSteamMatchmakingCallbacks + 10</summary>
        const int LobbyMatchList = 510;

        [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)] static extern IntPtr LoadLibraryW(string path);
        [DllImport("kernel32", CharSet = CharSet.Ansi)] static extern IntPtr GetProcAddress(IntPtr module, string name);

        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] delegate bool InitFn();
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void VoidFn();
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate IntPtr AccessorFn();
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate ulong RequestFn(IntPtr self);
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void StringFilterFn(IntPtr self, [MarshalAs(UnmanagedType.LPUTF8Str)] string key, [MarshalAs(UnmanagedType.LPUTF8Str)] string value, int comparison);
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate void IntFilterFn(IntPtr self, int value);
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate ulong ByIndexFn(IntPtr self, int index);
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate IntPtr DataFn(IntPtr self, ulong lobby, [MarshalAs(UnmanagedType.LPUTF8Str)] string key);
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] delegate bool CompletedFn(IntPtr self, ulong call, [MarshalAs(UnmanagedType.I1)] out bool failed);
        [UnmanagedFunctionPointer(CallingConvention.Cdecl)] [return: MarshalAs(UnmanagedType.I1)] delegate bool ResultFn(IntPtr self, ulong call, IntPtr buffer, int size, int expected, [MarshalAs(UnmanagedType.I1)] out bool failed);

        readonly IntPtr matchmaking, utils;
        readonly VoidFn shutdown, runCallbacks;
        readonly RequestFn request;
        readonly StringFilterFn stringFilter;
        readonly IntFilterFn distanceFilter, countFilter;
        readonly ByIndexFn byIndex;
        readonly DataFn data;
        readonly CompletedFn completed;
        readonly ResultFn result;
        bool disposed;

        SteamLobby(IntPtr module)
        {
            T Fn<T>(params string[] names) where T : Delegate
            {
                foreach (var name in names)
                {
                    var p = GetProcAddress(module, name);
                    if (p != IntPtr.Zero) return Marshal.GetDelegateForFunctionPointer<T>(p);
                }
                throw new InvalidOperationException("游戏的 Steam 接口缺少 " + names[0]);
            }
            IntPtr Interface(params string[] names)
            {
                var p = Fn<AccessorFn>(names)();
                if (p == IntPtr.Zero) throw new InvalidOperationException("Steam 接口未就绪");
                return p;
            }
            // 先取全部函数，确认这个版本的 steam_api64 都有，再登录 Steam
            var init = Fn<InitFn>("SteamAPI_Init");
            shutdown = Fn<VoidFn>("SteamAPI_Shutdown");
            runCallbacks = Fn<VoidFn>("SteamAPI_RunCallbacks");
            request = Fn<RequestFn>("SteamAPI_ISteamMatchmaking_RequestLobbyList");
            stringFilter = Fn<StringFilterFn>("SteamAPI_ISteamMatchmaking_AddRequestLobbyListStringFilter");
            distanceFilter = Fn<IntFilterFn>("SteamAPI_ISteamMatchmaking_AddRequestLobbyListDistanceFilter");
            countFilter = Fn<IntFilterFn>("SteamAPI_ISteamMatchmaking_AddRequestLobbyListResultCountFilter");
            byIndex = Fn<ByIndexFn>("SteamAPI_ISteamMatchmaking_GetLobbyByIndex");
            data = Fn<DataFn>("SteamAPI_ISteamMatchmaking_GetLobbyData");
            completed = Fn<CompletedFn>("SteamAPI_ISteamUtils_IsAPICallCompleted");
            result = Fn<ResultFn>("SteamAPI_ISteamUtils_GetAPICallResult");
            Environment.SetEnvironmentVariable("SteamAppId", AppId);
            Environment.SetEnvironmentVariable("SteamGameId", AppId);
            if (!init()) throw new InvalidOperationException("无法连接 Steam：请确认 Steam 已启动并登录，且账号拥有艾尔登法环");
            matchmaking = Interface("SteamAPI_SteamMatchmaking_v009");
            utils = Interface("SteamAPI_SteamUtils_v010", "SteamAPI_SteamUtils_v009");
        }

        /// <summary>
        /// 打开大厅连接。gameDir 为 Game 目录（eldenring.exe 所在）。装了无缝联机时原版接口可能改名为 steam_api64_o.dll，
        /// 这时 steam_api64.dll 是转发层，只加载原版那个。
        /// </summary>
        public static SteamLobby Open(string gameDir)
        {
            var original = Path.Combine(gameDir, "steam_api64_o.dll");
            var path = File.Exists(original) ? original : Path.Combine(gameDir, "steam_api64.dll");
            if (!File.Exists(path)) throw new InvalidOperationException("游戏目录里没有 steam_api64.dll");
            var module = LoadLibraryW(path);
            if (module == IntPtr.Zero) throw new InvalidOperationException($"无法加载 {Path.GetFileName(path)}（错误 {Marshal.GetLastWin32Error()}）");
            return new SteamLobby(module);
        }

        public bool ListLimited { get; private set; }
        public int ListQueries { get; private set; }
        int returnedCount;
        /// <summary>饱和列表分组查询并去重；精确房间名条件应用于每次请求。</summary>
        public async Task<List<LobbyRoom>> ListAsync(string exactName = "")
        {
            var found = await LobbyBrowse.CollectAsync(async filters => {
                var rooms = await QueryAsync(exactName, filters);
                return new LobbyBrowse.Page<LobbyRoom>(rooms, returnedCount >= 50);
            }, room => room.Id);
            ListLimited = found.Limited; ListQueries = found.Queries;
            Log.Write($"大厅汇总：queries={found.Queries} unique={found.Rooms.Count} limited={found.Limited}");
            return found.Rooms;
        }
        async Task<List<LobbyRoom>> QueryAsync(string exactName, IReadOnlyList<KeyValuePair<string,string>> filters)
        {
            if (disposed) throw new ObjectDisposedException(nameof(SteamLobby));
            stringFilter(matchmaking, "tower", Tag, 0);
            if (exactName.Length > 0) stringFilter(matchmaking, "name", exactName, 0);
            foreach (var filter in filters) stringFilter(matchmaking, filter.Key, filter.Value, 0);
            distanceFilter(matchmaking, 3);
            countFilter(matchmaking, MaxRooms);
            var call = request(matchmaking);
            if (call == 0) throw new InvalidOperationException("Steam 拒绝了大厅查询");
            bool failed = false;
            for (var i = 0; ; i++)
            {
                if (disposed) throw new ObjectDisposedException(nameof(SteamLobby));
                runCallbacks();
                if (completed(utils, call, out failed)) break;
                if (i >= 100) throw new TimeoutException("Steam 大厅查询超时");
                await Task.Delay(100);
            }
            var buffer = Marshal.AllocHGlobal(8);
            try
            {
                if (!result(utils, call, buffer, 4, LobbyMatchList, out failed) || failed) throw new InvalidOperationException("Steam 大厅查询失败");
                var returned = Marshal.ReadInt32(buffer);
                returnedCount = returned;
                var count = Math.Min(returned, MaxRooms);
                Log.Write($"大厅查询：mode={(exactName.Length > 0 ? "exact-name" : "browse")} requested={MaxRooms} returned={returned} read={count}");
                var rooms = new List<LobbyRoom>();
                for (var i = 0; i < count; i++)
                {
                    var id = byIndex(matchmaking, i);
                    if (id == 0 || Read(id, "tower") != Tag) continue;
                    rooms.Add(new LobbyRoom
                    {
                        Id = id, Name = Read(id, "name"), Host = Read(id, "host"), State = Read(id, "state"), Version = Read(id, "v"),
                        Players = int.TryParse(Read(id, "players"), out var p) ? p : 1, Max = int.TryParse(Read(id, "max"), out var m) ? m : 0,
                        Locked = Read(id, "lock") == "1", Open = Read(id, "open") == "1", Started = Read(id, "started") == "1", Salt = Read(id, "salt"), Password = Read(id, "pwd"),
                        PasswordFingerprint = Read(id, "pwd_sha256"),
                    });
                }
                return rooms;
            }
            finally { Marshal.FreeHGlobal(buffer); }
        }

        string Read(ulong lobby, string key)
        {
            var p = data(matchmaking, lobby, key);
            if (p == IntPtr.Zero) return "";
            var length = 0;
            while (length < 8192 && Marshal.ReadByte(p, length) != 0) length++;
            var bytes = new byte[length];
            Marshal.Copy(p, bytes, 0, length);
            return Encoding.UTF8.GetString(bytes);
        }

        public void Dispose()
        {
            if (disposed) return;
            disposed = true;
            shutdown();
        }

        // ---------------- 房间密码 ----------------

        static readonly char[] Alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789".ToCharArray();
        /// <summary>公开房间的无缝联机密码：16 位随机字母数字</summary>
        public static string RandomPassword()
        {
            var bytes = new byte[16];
            using (var rng = RandomNumberGenerator.Create()) rng.GetBytes(bytes);
            var text = new StringBuilder();
            foreach (var b in bytes) text.Append(Alphabet[b % Alphabet.Length]);
            return text.ToString();
        }
        /// <summary>私密房间的盐：32 位十六进制</summary>
        public static string RandomSalt()
        {
            var bytes = new byte[16];
            using (var rng = RandomNumberGenerator.Create()) rng.GetBytes(bytes);
            return Hex(bytes);
        }
        /// <summary>私密房间的无缝联机密码：SHA-256(盐 + ":" + 房间密码) 的前 24 位十六进制；大厅公开盐和派生密码的摘要</summary>
        public static string Derive(string salt, string roomPassword)
        {
            using var sha = SHA256.Create();
            return Hex(sha.ComputeHash(Encoding.UTF8.GetBytes(salt + ":" + roomPassword))).Substring(0, 24);
        }
        static string Hex(byte[] bytes)
        {
            var text = new StringBuilder(bytes.Length * 2);
            foreach (var b in bytes) text.Append(b.ToString("x2"));
            return text.ToString();
        }
    }
}
