using System.Text;
using System.Text.Json;
using Tower.Update;
using Xunit;

namespace Tower.Update.Tests;

public sealed class VersionSwitchTests : IDisposable
{
    readonly string root = Path.Combine(Path.GetTempPath(), "tower-switch-" + Guid.NewGuid().ToString("N"));
    public VersionSwitchTests() => Directory.CreateDirectory(root);
    public void Dispose() => Directory.Delete(root, true);

    ReleaseManifest Manifest(int version, params (string path, string text)[] rows)
    {
        var m = new ReleaseManifest { SchemaVersion = 1, Version = version, Tag = "v" + version, DisplayVersion = "2.0." + version, LauncherVersion = "1.0." + version };
        using var pack = new MemoryStream();
        foreach (var (path, text) in rows) {
            var bytes = Encoding.UTF8.GetBytes(text); var sha = Hashes.Of(bytes);
            m.Files.Add(new FileEntry { Path = path, Size = bytes.Length, Sha256 = sha });
            m.Blobs[sha] = new BlobLocation { Tag = m.Tag, Asset = "pack.bin", Offset = pack.Position, Length = bytes.Length, Size = bytes.Length, Method = "store" };
            pack.Write(bytes);
        }
        var folder = Path.Combine(root, m.Tag); Directory.CreateDirectory(folder);
        File.WriteAllBytes(Path.Combine(folder, "pack.bin"), pack.ToArray());
        File.WriteAllText(Path.Combine(folder, ReleaseManifest.FileName), JsonSerializer.Serialize(m));
        return m;
    }

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task DowngradeKeepsLauncherAndPrivateProgressAndCanRecoverFailure(bool fail)
    {
        var old = Manifest(1, ("old.dll", "old"), ("map.bin", "oldmap"), ("ErdtreeTowerLauncher.exe", "old launcher"));
        var current = Manifest(2, ("new.dll", "new"), ("map.bin", "newmap"), ("ErdtreeTowerLauncher.exe", "new launcher"), ("launcher/ui/new.js", "new UI"));
        var install = Path.Combine(root, "installed"); Directory.CreateDirectory(install);
        var source = new DirectoryReleaseSource(root);
        await new Updater(install, source).ApplyAsync(Planner.Plan(install, current, HashCache.Load(install)), null, default);
        File.WriteAllText(Path.Combine(install, "tower-0-0.json"), "private progress");
        var target = await source.VersionAsync("v1", default);
        var plan = VersionSwitch.Plan(install, target, HashCache.Load(install), "1.0.2");
        var updater = new Updater(install, source) { FailAfterWrites = fail ? 1 : null };
        if (fail) await Assert.ThrowsAsync<IOException>(() => updater.ApplyAsync(plan, null, default));
        else await updater.ApplyAsync(plan, null, default);
        Assert.Equal("new launcher", File.ReadAllText(Path.Combine(install, "ErdtreeTowerLauncher.exe")));
        Assert.Equal("new UI", File.ReadAllText(Path.Combine(install, "launcher/ui/new.js")));
        Assert.Equal("private progress", File.ReadAllText(Path.Combine(install, "tower-0-0.json")));
        Assert.Equal(fail ? "newmap" : "oldmap", File.ReadAllText(Path.Combine(install, "map.bin")));
        Assert.Equal(fail ? 2 : 1, InstallState.Load(install)!.Version);
        Assert.Equal(fail, File.Exists(Path.Combine(install, "new.dll")));
        if (!fail) Assert.True(VersionSwitch.Plan(install, old, HashCache.Load(install), "1.0.2").UpToDate);
    }

    [Theory]
    [InlineData("../v1")]
    [InlineData("v0")]
    [InlineData("v1/foo")]
    public async Task InvalidReleaseTagIsRejected(string tag) =>
        await Assert.ThrowsAsync<InvalidDataException>(() => new DirectoryReleaseSource(root).VersionAsync(tag, default));

    [Fact]
    public async Task HistoricalManifestMustMatchRequestedTag()
    {
        var m = Manifest(1, ("mod.dll", "test")); m.Version = 2;
        File.WriteAllText(Path.Combine(root, "v1", ReleaseManifest.FileName), JsonSerializer.Serialize(m));
        await Assert.ThrowsAsync<InvalidDataException>(() => new DirectoryReleaseSource(root).VersionAsync("v1", default));
    }

    [Fact]
    public async Task MissingMirrorFallsBackToExactHistoricalVersion()
    {
        Manifest(1, ("mod.dll", "old"));
        Manifest(2, ("mod.dll", "new"));
        var source = new MultiSource(new[] {
            new KeyValuePair<string, IReleaseSource>("missing", new DirectoryReleaseSource(Path.Combine(root, "missing"))),
            new KeyValuePair<string, IReleaseSource>("available", new DirectoryReleaseSource(root))
        });
        var target = await source.VersionAsync("v1", default);
        Assert.Equal(1, target.Version);
        Assert.Equal("available", source.Chosen);
        var blob = target.Blobs.Values.Single();
        Assert.Equal("old", Encoding.UTF8.GetString(await source.RangeAsync(blob.Tag, blob.Asset, blob.Offset, blob.Length, default)));
    }

    [Fact]
    public async Task CancelledHistoricalQueryDoesNotFallbackToLatest()
    {
        using var cancel = new CancellationTokenSource(); cancel.Cancel();
        await Assert.ThrowsAsync<OperationCanceledException>(() => new DirectoryReleaseSource(root).VersionAsync("v1", cancel.Token));
    }
}
