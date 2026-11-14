using Tower.Launcher;
using Xunit;

namespace Tower.Update.Tests;

public sealed class NetworkProfileTests
{
    [Fact]
    public void LobbyWorkerWirePreservesRoomIdentityAndAdmissionData()
    {
        var room = new LobbyRoom { Id = ulong.MaxValue, Name = "测试房间", Password = "secret", PasswordFingerprint = "hash", Code = "T1234567890ABCDEF", Players = 2, Max = 6, Open = true, Locked = true, Version = "test" };
        var json = System.Text.Json.JsonSerializer.Serialize(room, LobbyWire.Options);
        var copy = System.Text.Json.JsonSerializer.Deserialize<LobbyRoom>(json, LobbyWire.Options)!;
        Assert.Equal(room.Id, copy.Id);
        Assert.Equal(room.Name, copy.Name);
        Assert.Equal(room.Code, copy.Code);
        Assert.Equal(room.Password, copy.Password);
        Assert.Equal(room.PasswordFingerprint, copy.PasswordFingerprint);
        Assert.Equal(room.Max, copy.Max);
        Assert.Equal(room.Players, copy.Players);
        Assert.True(copy.Open && copy.Locked);
        Assert.Equal(room.Version, copy.Version);
    }

    [Fact]
    public void ModePersistsAndUnknownValuesUseOfficial()
    {
        var path = Path.GetTempFileName();
        try {
            Assert.Equal("Steam", new HostSettings().Network);
            var settings = new HostSettings { Network = "OnlineFix" };
            settings.Save(path);
            Assert.Equal("OnlineFix", HostSettings.Load(path).Network);
            File.WriteAllText(path, "{\"Network\":\"unexpected\"}");
            Assert.Equal("Steam", HostSettings.Load(path).Network);
        } finally { File.Delete(path); }
    }

    [Fact]
    public void ProfilesDoNotSharePathFilesOrLobbyApplications()
    {
        Assert.Equal("GamePath.txt", NetworkProfile.PathFile("Steam"));
        Assert.Equal("GamePath.OnlineFix.txt", NetworkProfile.PathFile("OnlineFix"));
        Assert.Equal("1245620", NetworkProfile.AppId("Steam"));
        Assert.Equal("480", NetworkProfile.AppId("OnlineFix"));
        Assert.Equal("1245620", NetworkProfile.AppId("invalid"));
        Assert.Equal("runtime", NetworkProfile.Runtime("Steam"));
        Assert.Equal("onlinefix-runtime", NetworkProfile.Runtime("OnlineFix"));
    }

    [Fact]
    public void DirectoryCannotBeSharedAcrossModes()
    {
        var path = Path.Combine(Path.GetTempPath(), "tower-network-test");
        Assert.Throws<InvalidOperationException>(() => NetworkProfile.AssertDifferent(path, path + Path.DirectorySeparatorChar));
        NetworkProfile.AssertDifferent(path, path + "-other");
        NetworkProfile.AssertDifferent(path, null);
    }
}
