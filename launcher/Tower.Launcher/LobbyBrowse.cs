using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;

namespace Tower.Launcher
{
    static class LobbyBrowse
    {
        public const int Capacity = 500;
        public sealed class Page<T>
        {
            public readonly IReadOnlyList<T> Rooms;
            public readonly bool Saturated;
            public Page(IReadOnlyList<T> rooms, bool saturated) { Rooms = rooms; Saturated = saturated; }
        }
        public sealed class Result<T>
        {
            public readonly List<T> Rooms = new List<T>();
            public bool Limited;
            public int Queries;
        }
        static readonly (string Key, string[] Values)[] Splits = {
            ("max", Enumerable.Range(2, 7).Select(n => n.ToString()).ToArray()),
            ("players", Enumerable.Range(1, 8).Select(n => n.ToString()).ToArray()),
            ("lock", new[] { "0", "1" }), ("started", new[] { "0", "1" }), ("open", new[] { "0", "1" })
        };
        public static async Task<Result<T>> CollectAsync<T>(Func<IReadOnlyList<KeyValuePair<string,string>>, Task<Page<T>>> query,
            Func<T,ulong> identity, Func<bool>? expired = null)
        {
            var clock = Stopwatch.StartNew();
            expired = expired ?? (() => clock.Elapsed.TotalSeconds >= 15);
            var result = new Result<T>();
            var indices = new Dictionary<ulong,int>();
            var pending = new Queue<List<KeyValuePair<string,string>>>();
            pending.Enqueue(new List<KeyValuePair<string,string>>());
            while (pending.Count > 0)
            {
                if (result.Queries >= 48 || result.Rooms.Count >= Capacity || result.Queries > 0 && expired()) { result.Limited = true; break; }
                var filters = pending.Dequeue();
                Page<T> page;
                try { page = await query(filters); }
                catch (TimeoutException) when (result.Queries > 0) { result.Limited = true; break; }
                result.Queries++;
                foreach (var room in page.Rooms)
                {
                    var id = identity(room);
                    if (indices.TryGetValue(id, out var index)) result.Rooms[index] = room;
                    else if (result.Rooms.Count < Capacity) { indices.Add(id, result.Rooms.Count); result.Rooms.Add(room); }
                    else result.Limited = true;
                }
                if (!page.Saturated) continue;
                if (filters.Count == Splits.Length) { result.Limited = true; continue; }
                var split = Splits[filters.Count];
                foreach (var value in split.Values)
                {
                    if (split.Key == "players" && int.Parse(value) > int.Parse(filters[0].Value)) continue;
                    var next = new List<KeyValuePair<string,string>>(filters) { new KeyValuePair<string,string>(split.Key, value) };
                    pending.Enqueue(next);
                }
            }
            return result;
        }
    }
}
