using Tower.Launcher;
using Xunit;

public class LobbyBrowseTests
{
    record Room(ulong Id, int Max, int Players = 1, string Lock = "0", string Started = "0", string Open = "1");
    static Task<LobbyBrowse.Page<Room>> Query(IEnumerable<Room> all, IReadOnlyList<KeyValuePair<string,string>> filters)
    {
        var matching = all.Where(r => filters.All(f => f.Value == (f.Key switch {
            "max" => r.Max.ToString(), "players" => r.Players.ToString(), "lock" => r.Lock, "started" => r.Started, "open" => r.Open, _ => throw new Exception()
        }))).Take(50).ToArray();
        return Task.FromResult(new LobbyBrowse.Page<Room>(matching, matching.Length == 50));
    }
    [Fact] public async Task Retrieves_beyond_the_first_fifty_without_duplicates()
    {
        var all = Enumerable.Range(0, 160).Select(i => new Room((ulong)i, 2 + i % 4)).ToArray();
        var result = await LobbyBrowse.CollectAsync(f => Query(all, f), r => r.Id, () => false);
        Assert.Equal(160, result.Rooms.Count); Assert.Equal(160, result.Rooms.Select(r => r.Id).Distinct().Count()); Assert.False(result.Limited);
    }
    [Fact] public async Task Recursively_splits_a_popular_room_capacity()
    {
        var all = Enumerable.Range(0, 120).Select(i => new Room((ulong)i, 4, 1 + i % 4)).ToArray();
        var result = await LobbyBrowse.CollectAsync(f => Query(all, f), r => r.Id, () => false);
        Assert.Equal(120, result.Rooms.Count); Assert.False(result.Limited);
    }
    [Fact] public async Task Identical_metadata_stays_bounded_and_reports_incomplete()
    {
        var all = Enumerable.Range(0, 90).Select(i => new Room((ulong)i, 4)).ToArray();
        var result = await LobbyBrowse.CollectAsync(f => Query(all, f), r => r.Id, () => false);
        Assert.Equal(50, result.Rooms.Count); Assert.True(result.Limited); Assert.InRange(result.Queries, 1, 48);
    }
    [Fact] public async Task Five_hundred_cap_and_time_budget_preserve_collected_results()
    {
        var all = Enumerable.Range(0, 700).Select(i => new Room((ulong)i, 2 + i % 7, 1 + i / 7 % 2)).ToArray();
        var capped = await LobbyBrowse.CollectAsync(f => Query(all, f), r => r.Id, () => false);
        Assert.Equal(500, capped.Rooms.Count); Assert.True(capped.Limited);
        var timed = await LobbyBrowse.CollectAsync(f => Query(all, f), r => r.Id, () => true);
        Assert.Equal(50, timed.Rooms.Count); Assert.True(timed.Limited); Assert.Equal(1, timed.Queries);
    }
    [Fact] public async Task Small_directory_needs_only_one_request()
    {
        var result = await LobbyBrowse.CollectAsync(f => Query(new[]{new Room(1,4)}, f), r => r.Id);
        Assert.Single(result.Rooms); Assert.Equal(1, result.Queries); Assert.False(result.Limited);
    }
}
