using Tower.Launcher;
using Xunit;

public class LauncherWindowSizeTests
{
    [Fact]
    public void SizePreferenceSurvivesRestartAndInvalidValuesFallBack()
    {
        var file = System.IO.Path.GetTempFileName();
        try
        {
            new HostSettings { WindowSize = "1600 × 900" }.Save(file);
            Assert.Equal("1600 × 900", HostSettings.Load(file).WindowSize);
            System.IO.File.WriteAllText(file, "{\"WindowSize\":null}");
            Assert.Equal(LauncherWindowSize.Default, HostSettings.Load(file).WindowSize);
        }
        finally { System.IO.File.Delete(file); }
    }
    [Fact]
    public void UnknownAndMissingSizesKeepOriginalDefault()
    {
        Assert.Equal(LauncherWindowSize.Default, LauncherWindowSize.Normalize(null));
        Assert.Equal(LauncherWindowSize.Default, LauncherWindowSize.Normalize("99999 × 0"));
        Assert.Equal((1280, 720), LauncherWindowSize.Fit(null, 96, 1920, 1080));
    }

    [Theory]
    [InlineData(96, 1920, 1080)]
    [InlineData(144, 1366, 728)]
    [InlineData(192, 800, 600)]
    public void EveryPresetFitsWorkAreaAtEachDpi(int dpi, int width, int height)
    {
        foreach (var value in LauncherWindowSize.Choices)
        {
            var size = LauncherWindowSize.Fit(value, dpi, width, height);
            Assert.InRange(size.Width, 1, width);
            Assert.InRange(size.Height, 1, height);
            Assert.InRange(System.Math.Abs(size.Width * 9 - size.Height * 16), 0, 16);
        }
    }
}
