using Xunit;

namespace Tower.Launcher {
    static class Log { public static void Write(string message) {} }
}
namespace Tower.Update.Tests {
    public sealed class LoaderSettingsTests {
        [Fact]
        public void LegacySoloSettingsUseUnifiedEntryWithoutChangingLoader() {
            var path=Path.GetTempFileName();
            try {
                File.WriteAllText(path,"{\"Mode\":\"Solo\",\"Loader\":\"YAFSML\"}");
                var settings=Tower.Launcher.HostSettings.Load(path);
                Assert.Equal("Seamless",settings.Mode);
                Assert.Equal("YAFSML",settings.Loader);
                Assert.Equal("Seamless",new Tower.Launcher.HostSettings().Mode);
            } finally {File.Delete(path);}
        }
        [Fact]
        public void LoaderDefaultsAndSelectionSurviveRestart() {
            var path=Path.GetTempFileName();
            try {
                File.WriteAllText(path,"{\"Mode\":\"Seamless\"}");
                var settings=Tower.Launcher.HostSettings.Load(path);
                Assert.Equal("ME3",settings.Loader);
                settings.Loader="YAFSML";settings.Save(path);
                var restored=Tower.Launcher.HostSettings.Load(path);
                Assert.Equal("YAFSML",restored.Loader);Assert.Equal("Seamless",restored.Mode);
                File.WriteAllText(path,"{\"Loader\":\"invalid\"}");
                Assert.Equal("ME3",Tower.Launcher.HostSettings.Load(path).Loader);
            } finally {File.Delete(path);}
        }
    }
}
