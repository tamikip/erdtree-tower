using Tower.Launcher;
using Xunit;

namespace Tower.Update.Tests;

public class LobbyLaunchTests
{
    [Fact]
    public void Wrong_private_password_and_missing_verifier_block_launch()
    {
        var hash = LobbyLaunch.Fingerprint("derived-correct-password");
        Assert.Null(LobbyLaunch.PasswordError(true, "derived-correct-password", hash));
        Assert.Null(LobbyLaunch.PasswordError(true, "derived-correct-password", hash.ToUpperInvariant()));
        Assert.Contains("密码错误", LobbyLaunch.PasswordError(true, "derived-wrong-password", hash));
        Assert.NotNull(LobbyLaunch.PasswordError(true, "anything", ""));
        Assert.NotNull(LobbyLaunch.PasswordError(true, "anything", new string('z', 64)));
        Assert.Null(LobbyLaunch.PasswordError(false, "legacy-public-password", ""));
        Assert.NotNull(LobbyLaunch.PasswordError(false, "stale-public-password", hash));
    }
    [Theory]
    [InlineData(true, false, false, false)]
    [InlineData(false, true, false, false)]
    [InlineData(false, false, true, false)]
    [InlineData(false, false, false, true)]
    public void Busy_launch_rejects_configuration_changes(bool game, bool launching, bool worker, bool updating)
    {
        Assert.NotNull(LobbyLaunch.BlockReason(game, launching, worker, updating));
    }

    [Fact]
    public void Idle_launch_is_allowed_and_password_fingerprint_is_exact()
    {
        Assert.Null(LobbyLaunch.BlockReason(false, false, false, false));
        Assert.Equal("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", LobbyLaunch.Fingerprint("abc"));
        Assert.NotEqual(LobbyLaunch.Fingerprint("abc"), LobbyLaunch.Fingerprint("Abc"));
    }
}
