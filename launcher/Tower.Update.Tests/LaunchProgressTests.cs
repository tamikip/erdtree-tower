using Tower.Launcher;
using System.Text;
using Xunit;

public class LaunchProgressTests
{
    [Fact]
    public void CountsArePhaseLocalAndUnmeasuredStagesHaveNoInventedPercent()
    {
        var p = new LaunchProgress(); Assert.Null(p.Percent);
        p.Output("TOWER_PROGRESS|building|1|3|正在生成本地资源|游戏特效资源 · 压缩生成结果");
        Assert.Equal("building", p.Phase); Assert.Equal(100.0 / 3, p.Percent);
        p.Output("TOWER_PROGRESS|icons|25|100|正在生成物品图标|保存"); Assert.Equal(25, p.Percent);
        p.Output("TOWER_PROGRESS|starting|0|0|正在启动游戏|ME3"); Assert.Null(p.Percent);
    }
    [Fact]
    public void InvalidMessagesCannotChangeProgress()
    {
        var p = new LaunchProgress();
        foreach (var line in new[] { "random output", "TOWER_PROGRESS|building|5|3|bad|bad", "TOWER_PROGRESS|building|-1|3|bad|bad", "TOWER_PROGRESS|finished|1|1|bad|bad" }) p.Output(line);
        Assert.Equal("preparing", p.Phase);
    }
    [Fact]
    public void ANewNativeSessionIsRequiredBeforeReportingFirstFrame()
    {
        var p = new LaunchProgress(); p.Native("First overlay frame rendered"); Assert.False(p.FrameReady);
        p.GameDetected(); Assert.Equal("waiting", p.Phase); Assert.False(p.Terminal);
        p.Native("SESSION_START build=test"); p.Native("NATIVE_SETTINGS registration result=0");
        p.Output("TOWER_PROGRESS|checking|0|3|old resource message|old"); Assert.Equal("waiting", p.Phase);
        p.Native("First overlay frame rendered"); Assert.True(p.FrameReady); Assert.True(p.Terminal);
        p.Native("SESSION_START stale"); Assert.Equal("finished", p.Phase);
    }
    [Fact]
    public void FailureRemainsVisibleAndCanOverrideAnEarlierFrame()
    {
        var p = new LaunchProgress(); p.Native("SESSION_START"); p.Native("First overlay frame rendered");
        p.End("failed", "启动失败", "<error>"); p.Output("TOWER_PROGRESS|building|0|2|stale|stale");
        Assert.Equal("failed", p.Phase); Assert.Equal("<error>", p.Detail); Assert.True(p.Terminal);
    }
    [Fact]
    public void TailSkipsOldSessionsAndKeepsSplitUtf8AndPartialLines()
    {
        var dir = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N")); Directory.CreateDirectory(dir);
        var path = Path.Combine(dir, "log.txt"); File.WriteAllText(path, "old session\n", new UTF8Encoding(false));
        try {
            var reader = new LaunchLogReader(path, true); Assert.Empty(reader.Read());
            var bytes = Encoding.UTF8.GetBytes("正在生成\n");
            using (var s = new FileStream(path, FileMode.Append)) s.Write(bytes, 0, 2);
            Assert.Empty(reader.Read());
            using (var s = new FileStream(path, FileMode.Append)) s.Write(bytes, 2, bytes.Length - 2);
            Assert.Equal(new[] { "正在生成" }, reader.Read()); Assert.Empty(reader.Read());
            File.WriteAllText(path, "new\n", new UTF8Encoding(false)); Assert.Equal(new[] { "new" }, reader.Read());
        } finally { Directory.Delete(dir, true); }
    }
    [Fact]
    public void MissingLogIsRetriedWhenItAppears()
    {
        var path = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        var reader = new LaunchLogReader(path); Assert.Empty(reader.Read());
        try { File.WriteAllText(path, "ready\n"); Assert.Equal(new[] { "ready" }, reader.Read()); }
        finally { File.Delete(path); }
    }
}
