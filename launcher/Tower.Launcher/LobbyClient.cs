using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows.Forms;

namespace Tower.Launcher
{
    // Each query owns its Steam SDK process, so switching profiles cannot reuse a loaded DLL.
    sealed class LobbyClient : IDisposable
    {
        public sealed class Request
        {
            public string GameDir { get; set; } = "";
            public string Network { get; set; } = NetworkProfile.Steam;
            public string Action { get; set; } = "list";
            public string Value { get; set; } = "";
            public LobbyRoom? Room { get; set; }
        }
        public sealed class Response
        {
            public List<LobbyRoom> Rooms { get; set; } = new List<LobbyRoom>();
            public bool Limited { get; set; }
            public string? Error { get; set; }
        }

        readonly string gameDir, network;
        Process? worker;
        bool disposed;
        public bool ListLimited { get; private set; }
        public LobbyClient(string gameDir, string network) { this.gameDir = gameDir; this.network = network; }
        public Task<List<LobbyRoom>> ListAsync(string query) => QueryAsync("list", query);
        public Task<List<LobbyRoom>> QuickRoomsAsync(string version) => QueryAsync("quick", version);
        public async Task<LobbyRoom?> FindCodeAsync(string code) => First(await QueryAsync("code", code));
        public async Task<LobbyRoom?> RefreshAsync(LobbyRoom room) => First(await QueryAsync("refresh", "", room));
        static LobbyRoom? First(List<LobbyRoom> rooms) => rooms.Count == 0 ? null : rooms[0];

        async Task<List<LobbyRoom>> QueryAsync(string action, string value, LobbyRoom? room = null)
        {
            if (disposed) throw new ObjectDisposedException(nameof(LobbyClient));
            if (worker != null) throw new InvalidOperationException("大厅查询尚未完成");
            var info = new ProcessStartInfo(Application.ExecutablePath, "--lobby-worker") {
                UseShellExecute = false, CreateNoWindow = true,
                RedirectStandardInput = true, RedirectStandardOutput = true,
                StandardOutputEncoding = Encoding.UTF8, WorkingDirectory = gameDir,
            };
            using var process = new Process { StartInfo = info };
            worker = process;
            try {
                process.Start();
                var output = process.StandardOutput.ReadToEndAsync();
                await process.StandardInput.WriteAsync(JsonSerializer.Serialize(new Request {
                    GameDir = gameDir, Network = network, Action = action, Value = value, Room = room,
                }, LobbyWire.Options));
                process.StandardInput.Close();
                if (await Task.WhenAny(output, Task.Delay(TimeSpan.FromMinutes(3))) != output)
                    throw new TimeoutException("大厅查询超时，请检查 Steam 和网络连接");
                if (disposed) throw new ObjectDisposedException(nameof(LobbyClient));
                var response = JsonSerializer.Deserialize<Response>(await output, LobbyWire.Options)
                    ?? throw new InvalidOperationException("大厅查询进程未返回结果");
                if (response.Error != null) throw new InvalidOperationException(response.Error);
                ListLimited = response.Limited;
                return response.Rooms;
            } catch (JsonException) {
                throw new InvalidOperationException("大厅查询进程退出或返回异常，请确认 Steam 已启动");
            } catch (IOException e) {
                throw new InvalidOperationException("大厅查询进程连接失败：" + e.Message);
            } catch (System.ComponentModel.Win32Exception e) {
                throw new InvalidOperationException("无法启动大厅查询进程：" + e.Message);
            } finally {
                Stop(process);
                worker = null;
            }
        }

        static void Stop(Process process)
        {
            try { if (!process.HasExited) process.Kill(); }
            catch (InvalidOperationException) { }
            catch (System.ComponentModel.Win32Exception) { }
        }
        public void Dispose() { disposed = true; if (worker != null) Stop(worker); }

        public static async Task RunWorkerAsync()
        {
            var response = new Response();
            try {
                using var input = new StreamReader(Console.OpenStandardInput(), Encoding.UTF8);
                var request = JsonSerializer.Deserialize<Request>(await input.ReadToEndAsync(), LobbyWire.Options)
                    ?? throw new InvalidOperationException("Missing lobby request");
                if (request.Network != NetworkProfile.Steam && request.Network != NetworkProfile.OnlineFix)
                    throw new InvalidOperationException("Unknown network profile");
                using var steam = SteamLobby.Open(request.GameDir, NetworkProfile.AppId(request.Network));
                switch (request.Action) {
                    case "list": response.Rooms = await steam.ListAsync(request.Value); break;
                    case "quick": response.Rooms = await steam.QuickRoomsAsync(request.Value); break;
                    case "code":
                        var found = await steam.FindCodeAsync(request.Value);
                        if (found != null) response.Rooms.Add(found);
                        break;
                    case "refresh":
                        if (request.Room == null) throw new InvalidOperationException("Missing room");
                        var refreshed = await steam.RefreshAsync(request.Room);
                        if (refreshed != null) response.Rooms.Add(refreshed);
                        break;
                    default: throw new InvalidOperationException("Unknown lobby action");
                }
                response.Limited = steam.ListLimited;
            } catch (Exception e) { response.Error = e.Message; }
            using var output = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false));
            await output.WriteAsync(JsonSerializer.Serialize(response, LobbyWire.Options));
        }
    }
}
