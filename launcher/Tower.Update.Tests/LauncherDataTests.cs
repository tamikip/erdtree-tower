using Tower.Launcher;
using Xunit;

namespace Tower.Update.Tests;

public sealed class LauncherDataTests : IDisposable
{
    readonly string root = Path.Combine(Path.GetTempPath(), "tower-data-test-" + Guid.NewGuid().ToString("N"));
    string Preferred => Path.Combine(root, "preferred");
    string Fallback => Path.Combine(root, "fallback");

    [Fact]
    public void Writable_existing_data_is_preserved()
    {
        Directory.CreateDirectory(Preferred);
        File.WriteAllText(Path.Combine(Preferred, "settings.json"), "settings");
        Assert.Equal(Preferred, LauncherData.Select(Preferred, Fallback));
        Assert.False(Directory.Exists(Fallback));
        Assert.Equal("settings", File.ReadAllText(Path.Combine(Preferred, "settings.json")));
    }

    [Fact]
    public void Blocked_launch_directory_falls_back_and_preserves_settings()
    {
        Directory.CreateDirectory(Preferred);
        File.WriteAllText(Path.Combine(Preferred, "launches"), "blocked");
        File.WriteAllText(Path.Combine(Preferred, "settings.json"), "settings");
        Assert.Equal(Fallback, LauncherData.Select(Preferred, Fallback));
        Assert.Equal("settings", File.ReadAllText(Path.Combine(Fallback, "settings.json")));
        File.WriteAllText(Path.Combine(Fallback, "settings.json"), "updated");
        File.Delete(Path.Combine(Preferred, "launches"));
        Assert.Equal(Fallback, LauncherData.Select(Preferred, Fallback));
        Assert.Equal("updated", File.ReadAllText(Path.Combine(Fallback, "settings.json")));
    }

    [Fact]
    public void Locked_log_falls_back_without_truncating_it()
    {
        Directory.CreateDirectory(Preferred);
        var log = Path.Combine(Preferred, "launcher.log");
        File.WriteAllText(log, "keep");
        using (File.Open(log, FileMode.Open, FileAccess.Read, FileShare.Read))
            Assert.Equal(Fallback, LauncherData.Select(Preferred, Fallback));
        Assert.Equal("keep", File.ReadAllText(log));
    }

    public void Dispose() { if (Directory.Exists(root)) Directory.Delete(root, true); }
}
