using System.Text.Json;

namespace Tower.Launcher
{
    static class LobbyWire
    {
        public static readonly JsonSerializerOptions Options = new JsonSerializerOptions { IncludeFields = true };
    }
}
