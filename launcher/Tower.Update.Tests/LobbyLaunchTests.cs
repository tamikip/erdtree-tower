using Tower.Launcher;
using Xunit;

namespace Tower.Update.Tests;

public class LobbyLaunchTests
{
    static LobbyRoom Room(ulong id=1, int players=1) => new LobbyRoom {
        Id=id, Name="room", Version="2.1.0", Players=players, Max=4, Open=true,
        Password="automatic-connection-secret", PasswordFingerprint=LobbyLaunch.Fingerprint("automatic-connection-secret")
    };
    [Fact]
    public void Quick_join_only_selects_open_compatible_public_rooms()
    {
        var closed=Room(1); closed.Open=false;
        var privateRoom=Room(2); privateRoom.Locked=true;
        var started=Room(3); started.Started=true;
        var wrong=Room(4); wrong.Version="old";
        var full=Room(5,4);
        var invalid=Room(6,0);
        var free=Room(7); var nearlyFull=Room(8,3);
        var rooms=new[]{closed,privateRoom,started,wrong,full,invalid,free,nearlyFull};
        Assert.Equal(8ul,LobbyLaunch.QuickCandidate(rooms,"2.1.0")?.Id);
        Assert.Null(LobbyLaunch.QuickCandidate(rooms.Take(6),"2.1.0"));
        Assert.NotNull(LobbyLaunch.JoinError(closed,"2.1.0"));
        Assert.Null(LobbyLaunch.JoinError(privateRoom,"2.1.0"));
    }
    [Theory]
    [InlineData("T-0123-4567-89AB-CDEF", "T0123456789ABCDEF")]
    [InlineData(" t0123456789abcdef ", "T0123456789ABCDEF")]
    [InlineData("", null)]
    [InlineData("1234", null)]
    [InlineData("T0123456789ABCDEG", null)]
    [InlineData("T0123456789ABCDEFF", null)]
    public void Room_codes_have_one_canonical_form(string text,string? expected)
        => Assert.Equal(expected,LobbyLaunch.NormalizeRoomCode(text));
    [Fact]
    public void Room_codes_are_generated_independently_of_connection_passwords()
    {
        var codes=Enumerable.Range(0,100).Select(_=>LobbyLaunch.NewRoomCode()).ToArray();
        Assert.Equal(codes.Length,codes.Distinct().Count());
        Assert.All(codes,code=>Assert.Equal(code,LobbyLaunch.NormalizeRoomCode(code)));
    }
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
