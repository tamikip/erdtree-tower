using Tower.Launcher;
using Xunit;

public class DlcLaunchCheckTests
{
    [Theory]
    [InlineData(false, "none", false)]
    [InlineData(false, "weapons", false)]
    [InlineData(false, "full", true)]
    [InlineData(false, null, true)]
    [InlineData(false, "invalid", true)]
    [InlineData(true, "none", false)]
    [InlineData(true, "weapons", false)]
    [InlineData(true, "full", true)]
    public void MissingAssetsWarnOnlyWhenFullContentIsEnabled(bool seamless, string? mode, bool warn)
    {
        Assert.Equal(warn, DlcLaunchCheck.Warning("unused", seamless, mode, _ => false) != null);
    }

    [Fact]
    public void BothArchivesAreRequired()
    {
        Assert.NotNull(DlcLaunchCheck.Warning("unused", false, "full", p => p.EndsWith(".bhd")));
        Assert.NotNull(DlcLaunchCheck.Warning("unused", false, "full", p => p.EndsWith(".bdt")));
        Assert.Null(DlcLaunchCheck.Warning("unused", false, "full", _ => true));
    }

    [Fact]
    public void NoDlcNeverWarnsEvenInSeamless()
    {
        Assert.Null(DlcLaunchCheck.Warning("unused", true, "none", _ => true));
        Assert.Null(DlcLaunchCheck.Warning("unused", true, "weapons", _ => false));
    }

    [Fact]
    public void EmptyArchivesAreMissing()
    {
        var dir = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        try {
            File.WriteAllBytes(Path.Combine(dir, "DLC.bhd"), Array.Empty<byte>());
            File.WriteAllBytes(Path.Combine(dir, "DLC.bdt"), new byte[] { 1 });
            Assert.NotNull(DlcLaunchCheck.Warning(dir, false, "full"));
            File.WriteAllBytes(Path.Combine(dir, "DLC.bhd"), new byte[] { 1 });
            Assert.Null(DlcLaunchCheck.Warning(dir, false, "full"));
        } finally { Directory.Delete(dir, true); }
    }
}
