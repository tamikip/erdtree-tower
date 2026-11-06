using System.Text;
using Tower.Update;
using Xunit;

/// <summary>无缝联机配置：只改已有的、启动器管理的项，注释与其他行保留，第一次修改前备份</summary>
public sealed class SeamlessSettingsTests : IDisposable
{
    readonly string game = Path.Combine(Path.GetTempPath(), "tower-seamless-tests-" + Guid.NewGuid().ToString("N"));
    const string Sample = "﻿; Seamless Co-op settings\r\n[GAMEPLAY]\r\nallow_invaders = 1\r\ndeath_debuffs = 1\r\nallow_summons = 1\r\n\r\n[SCALING]\r\nenemy_health_scaling = 35\r\nboss_health_scaling = 100\r\n\r\n[PASSWORD]\r\ncooppassword = \r\n\r\n[SAVE]\r\nsave_file_extension = co2\r\n";

    public SeamlessSettingsTests()
    {
        Directory.CreateDirectory(Path.Combine(game, "SeamlessCoop"));
        File.WriteAllText(Path.Combine(game, "SeamlessCoop", "ersc_settings.ini"), Sample, new UTF8Encoding(false));
        File.WriteAllText(Path.Combine(game, "SeamlessCoop", "ersc.dll"), "not the verified build");
    }

    public void Dispose() { try { Directory.Delete(game, true); } catch (IOException) { } }

    [Fact]
    public void BundledSettingsKeepPasswordPrivateAndPreserveExistingRuntime() {
        var package=Path.Combine(game,"package");
        Directory.CreateDirectory(Path.Combine(package,"SeamlessCoop"));
        File.WriteAllText(Path.Combine(package,"SeamlessCoop","ersc.dll"),"test");
        File.WriteAllText(Path.Combine(package,"SeamlessCoop","default.ini"),Sample);
        WritePreset(package);
        var runtime=Path.Combine(package,"seamless-entry","runtime","SeamlessCoop");
        Directory.CreateDirectory(runtime);
        File.WriteAllText(Path.Combine(runtime,"ersc_settings.ini"),Sample.Replace("cooppassword = ","cooppassword = existing"));
        var original=File.ReadAllBytes(Path.Combine(game,"SeamlessCoop","ersc_settings.ini"));
        var settings=SeamlessSettings.ForPackage(package);
        Assert.True(settings.Installed);
        Assert.Equal("existing",settings.Read()["cooppassword"]);
        settings.Write("cooppassword","new-password");
        Assert.Equal("new-password",SeamlessSettings.ForPackage(package).Read()["cooppassword"]);
        Assert.Equal(original,File.ReadAllBytes(Path.Combine(game,"SeamlessCoop","ersc_settings.ini")));
        Assert.DoesNotContain("new-password",File.ReadAllText(Path.Combine(package,"SeamlessCoop","default.ini")));
    }

    [Fact]
    public void FreshPackageGetsItsOwnPassword() {
        var package=Path.Combine(game,"fresh");
        Directory.CreateDirectory(Path.Combine(package,"SeamlessCoop"));
        File.WriteAllText(Path.Combine(package,"SeamlessCoop","default.ini"),Sample);
        WritePreset(package);
        var settings=SeamlessSettings.ForPackage(package);
        Assert.Equal(32,settings.Read()["cooppassword"].Length);
        Assert.Equal(settings.Read()["cooppassword"],SeamlessSettings.ForPackage(package).Read()["cooppassword"]);
    }

    static void WritePreset(string package) => File.WriteAllText(Path.Combine(package,"seamless-preset.ini"),
        "[GAMEPLAY]\nallow_summons = 1\n[SCALING]\n"+string.Join("\n",SeamlessSettings.Scalings.Select(key=>key+" = 0")));

    [Fact]
    public void TowerPresetIsDisplayedAndEditedWithoutChangingOriginalScaling()
    {
        var preset=Path.Combine(game,"seamless-preset.ini");
        File.WriteAllText(preset,"[GAMEPLAY]\nallow_summons = 1\ndeath_debuffs = 0\n[SCALING]\nenemy_health_scaling = 100\nenemy_damage_scaling = 20\nenemy_posture_scaling = 30\nboss_health_scaling = 100\nboss_damage_scaling = 20\nboss_posture_scaling = 80\n");
        var settings=new SeamlessSettings(game,preset);
        var original=File.ReadAllBytes(settings.IniFile);
        var values=settings.Read();
        Assert.Equal(new[]{"100","20","30","100","20","80"},SeamlessSettings.Scalings.Select(k=>values[k]).ToArray());
        Assert.Equal("0",values["death_debuffs"]);
        settings.Write("enemy_health_scaling","125");
        Assert.Equal("125",settings.Read()["enemy_health_scaling"]);
        Assert.Contains("enemy_health_scaling = 125",File.ReadAllText(preset));
        Assert.Equal(original,File.ReadAllBytes(settings.IniFile));
        Assert.Throws<InvalidOperationException>(()=>settings.Write("death_debuffs","1"));
        File.Delete(preset);
        Assert.Throws<IOException>(()=>settings.Read());
    }

    [Fact]
    public void EditsKeepEverythingElseAndBackUpTheOriginalOnce()
    {
        var settings = new SeamlessSettings(game);
        settings.Write("cooppassword", "  tower42 ");
        settings.Write("death_debuffs", "0");
        settings.Write("enemy_health_scaling", "50");
        var text = File.ReadAllText(settings.IniFile);
        Assert.Contains("; Seamless Co-op settings", text);
        Assert.Contains("allow_invaders = 1", text);
        Assert.Contains("cooppassword = tower42\r\n", text);
        Assert.Contains("death_debuffs = 0", text);
        Assert.Contains("enemy_health_scaling = 50", text);
        Assert.Equal(new byte[] { 0xef, 0xbb, 0xbf }, File.ReadAllBytes(settings.IniFile).Take(3));
        Assert.Equal(Sample, File.ReadAllText(settings.IniFile + SeamlessSettings.BackupSuffix, Encoding.UTF8).Insert(0, "﻿"));
        var values = settings.Read();
        Assert.Equal("tower42", values["cooppassword"]);
        Assert.Equal("0", values["death_debuffs"]);
    }

    [Theory]
    [InlineData("cooppassword", "")]
    [InlineData("cooppassword", "a;b")]
    [InlineData("death_debuffs", "2")]
    [InlineData("enemy_health_scaling", "-5")]
    [InlineData("save_file_extension", "sl2")]
    [InlineData("allow_invaders", "1")]
    [InlineData("mod_language", "english")]
    [InlineData("boss_damage_scaling", "10")]  // 示例配置里没有这一项：不新增
    public void InvalidValuesForcedKeysAndMissingKeysAreRefused(string key, string value) =>
        Assert.Throws<InvalidOperationException>(() => new SeamlessSettings(game).Write(key, value));

    [Fact]
    public void AnUnverifiedDllIsReportedAndAMissingOneMeansNotInstalled()
    {
        var settings = new SeamlessSettings(game);
        var (installed, verified, label) = settings.Version();
        Assert.True(installed);
        Assert.False(verified);
        Assert.Contains("未验证：可以启动", label);
        File.Delete(settings.Dll);
        Assert.False(settings.Version().Installed);
    }
}
