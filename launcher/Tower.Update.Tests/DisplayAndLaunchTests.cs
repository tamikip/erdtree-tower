using Tower.Update;
using Tower.Launcher;
using Xunit;

namespace Tower.Update.Tests;

public sealed class DisplayAndLaunchTests
{
    [Fact]
    public void Player_labels_hide_internal_builds()
    {
        Assert.Equal("1.6.1", Versions.PlayerLabel("1.6.1"));
        Assert.Equal("历史版本", Versions.PlayerLabel(null));
        Assert.Equal("历史版本", Versions.PlayerLabel("v106"));
        Assert.Equal("v106", Versions.Label(106, null));
    }
    [Fact]
    public void Display_version_survives_install_state_and_legacy_falls_back()
    {
        var root=Path.Combine(Path.GetTempPath(),Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try {
            new InstallState {Version=97,DisplayVersion="1.2.3"}.Save(root);
            var state=InstallState.Load(root)!;
            Assert.Equal(97,state.Version);
            Assert.Equal("1.2.3",Versions.Label(state.Version,state.DisplayVersion));
            File.WriteAllText(Path.Combine(root,InstallState.FileName),"{\"version\":96,\"files\":[]}");
            state=InstallState.Load(root)!;
            Assert.Equal("v96",Versions.Label(state.Version,state.DisplayVersion));
            Assert.False(Versions.Valid("1.02.0"));
            Assert.False(Versions.Valid("1.0.0\n"));
        } finally {Directory.Delete(root,true);}
    }

    [Fact]
    public void Launch_summary_prefers_script_error_and_captures_native_stderr()
    {
        var root=Path.Combine(Path.GetTempPath(),Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try {
            var error=Path.Combine(root,"error.txt");
            File.WriteAllText(Path.Combine(root,"stderr.log"),"INFO starting\nERROR loader could not load plugin\nINFO cleanup\n");
            Assert.Equal("ERROR loader could not load plugin",LaunchResult.Summary(error,root,1));
            File.WriteAllText(error,"缺少文件");
            Assert.Equal("缺少文件",LaunchResult.Summary(error,root,1));
            File.Delete(error);File.Delete(Path.Combine(root,"stderr.log"));
            Assert.Contains("42",LaunchResult.Summary(error,root,42));
        } finally {Directory.Delete(root,true);}
    }
}
