using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;

namespace Tower.Update
{
    /// <summary>
    /// 包内 Seamless Co-op：组件与私人配置分开，更新不覆盖玩家密码。
    /// 只改已经存在的项（不同版本的无缝联机字段不完全一样），注释和其他行原样保留，第一次修改前留一份备份。
    /// 无缝入口（Launch-Seamless-Tower.ps1）启动时复制这份配置，再强制改存档后缀、关入侵、开死亡观战。
    /// </summary>
    public sealed class SeamlessSettings
    {
        /// <summary>
        /// 放行名单里的 Seamless Co-op（ersc.dll 的 SHA-256，与 Launch-Seamless-Tower.ps1 的 $verifiedSeamless 一致）：
        /// 名单里的不提醒；名单外的提示版本差异。
        /// </summary>
        public static readonly (string Sha256, string Version, bool Revival)[] Known =
        {
            ("fcd11a1849fd37851bb449be30f1525cf498f63fd1fa568c4e15030ebe7cb251", "2.0.1", true),
            ("0a093d8490967bf19c62f895fdf1fda21840282d2b2b371b9b4fc24a0cfe146e", "2.0.2", true),
            ("2ebde62be864ba3d759728aac44c459f0f813e8b6d7a342304ce68f6513f280c", "2.0.3", true),
        };
        public const string BackupSuffix = ".tower-backup";

        /// <summary>启动器能改的项：开关（0/1）与数值（0–1000 的百分比）</summary>
        public static readonly string[] Switches = { "death_debuffs", "allow_summons", "skip_splash_screens" };
        public static readonly string[] Scalings =
        {
            "enemy_health_scaling", "enemy_damage_scaling", "enemy_posture_scaling",
            "boss_health_scaling", "boss_damage_scaling", "boss_posture_scaling",
        };
        public const string Password = "cooppassword";
        /// <summary>爬塔的无缝入口启动时会强制改掉的项：在这里改了也不生效</summary>
        public static readonly string[] Forced = { "save_file_extension", "allow_invaders", "always_spectate_on_death" };

        readonly string folder;
        readonly string? presetFile;
        string? iniFile;
        public SeamlessSettings(string gameDir, string? presetFile = null) { folder = Path.Combine(gameDir, "SeamlessCoop"); this.presetFile = presetFile; }

        public static SeamlessSettings ForPackage(string root)
        {
            var settings=new SeamlessSettings(root,Path.Combine(root,"seamless-preset.ini"));
            settings.iniFile=Path.Combine(root,"config","SeamlessCoop","ersc_settings.ini");
            if (!File.Exists(settings.iniFile)) {
                var previous=Path.Combine(root,"seamless-entry","runtime","SeamlessCoop","ersc_settings.ini");
                var template=Path.Combine(root,"SeamlessCoop","default.ini");
                var source=File.Exists(previous)?previous:template;
                if (File.Exists(source)) {
                    var text=File.ReadAllText(source);
                    if (source==template) text=Line(Password).Replace(text,m=>m.Groups[1].Value+" "+Guid.NewGuid().ToString("N"),1);
                    Directory.CreateDirectory(Path.GetDirectoryName(settings.iniFile)!);
                    try {
                        using(var stream=new FileStream(settings.iniFile,FileMode.CreateNew,FileAccess.Write))
                        using(var writer=new StreamWriter(stream,new UTF8Encoding(false))) writer.Write(text);
                    } catch(IOException) when(File.Exists(settings.iniFile)) { }
                }
            }
            return settings;
        }

        public string Folder => iniFile == null ? folder : Path.GetDirectoryName(iniFile)!;
        public string IniFile => iniFile ?? Path.Combine(folder, "ersc_settings.ini");
        public string Dll => Path.Combine(folder, "ersc.dll");
        public bool Installed => File.Exists(Dll) && File.Exists(IniFile);

        /// <summary>安装状态与版本：哈希等于验证过的 2.0.1 时标为已验证；其他版本也能启动，复活修复会自动关闭</summary>
        public (bool Installed, bool Verified, string Label) Version()
        {
            if (!File.Exists(Dll)) return (false, false, "缺少包内 SeamlessCoop\\ersc.dll，请校验并修复文件");
            if (!File.Exists(IniFile)) return (false, false, "缺少无缝配置，请校验并修复文件");
            string sha;
            using (var stream = File.OpenRead(Dll)) sha = Hashes.Of(stream);
            foreach (var known in Known)
                if (sha == known.Sha256)
                    return (true, true, known.Version == "2.0.3" ? "2.0.3（包内配套版本，复活接口已适配）" : known.Revival ? $"{known.Version}（本包验证过的版本）" : $"{known.Version}（已放行；联机复活修复尚未适配，会自动关闭）");
            var info = FileVersionInfo.GetVersionInfo(Dll);
            var version = string.IsNullOrWhiteSpace(info.ProductVersion) ? info.FileVersion : info.ProductVersion;
            var shown = string.IsNullOrWhiteSpace(version) ? "未知版本" : version!.Trim();
            return (true, false, $"{shown}（未验证：可以启动，联机复活修复会自动关闭；联机双方须同一版本）");
        }

        static Regex Line(string key) => new Regex($@"(?m)^([ \t]*{Regex.Escape(key)}[ \t]*=)[^\r\n]*");

        /// <summary>文件里出现的 键 = 值（只取第一次出现；名字区分大小写，与无缝联机一致）</summary>
        public Dictionary<string, string> Read()
        {
            var values=ReadFile(IniFile);
            if (presetFile != null) {
                if (!File.Exists(presetFile)) throw new IOException("缺少 seamless-preset.ini，请完整更新爬塔文件");
                var preset=ReadFile(presetFile);
                foreach(var key in Scalings.Concat(new[]{"allow_summons"})) {
                    if (!preset.TryGetValue(key,out var value)) throw new IOException("爬塔预设缺少 " + key);
                    values[key]=Validate(key,value);
                }
                values["death_debuffs"]="0";
                values["always_spectate_on_death"]="1";
                values["allow_invaders"]="0";
            }
            return values;
        }

        static Dictionary<string,string> ReadFile(string path)
        {
            var values = new Dictionary<string, string>();
            if (!File.Exists(path)) return values;
            foreach (Match m in Regex.Matches(File.ReadAllText(path), @"(?m)^[ \t]*([A-Za-z0-9_]+)[ \t]*=[ \t]*([^\r\n;#]*)"))
                if (!values.ContainsKey(m.Groups[1].Value)) values[m.Groups[1].Value] = m.Groups[2].Value.Trim();
            return values;
        }

        /// <summary>改一项：只允许启动器管理的项，值先校验；文件里没有这一项时不新增（版本不同的无缝联机可能不认）</summary>
        public void Write(string key, string value)
        {
            value = Validate(key, value);
            if (presetFile != null && key=="death_debuffs") throw new InvalidOperationException("爬塔固定关闭死亡减益");
            var path=presetFile != null && (Scalings.Contains(key)||key=="allow_summons") ? presetFile : IniFile;
            if (!File.Exists(path)) throw new InvalidOperationException("没找到配置文件："+Path.GetFileName(path));
            var raw = File.ReadAllBytes(path);
            var bom = raw.Length >= 3 && raw[0] == 0xef && raw[1] == 0xbb && raw[2] == 0xbf;
            var text = new UTF8Encoding(false).GetString(raw, bom ? 3 : 0, raw.Length - (bom ? 3 : 0));
            var line = Line(key);
            if (!line.IsMatch(text)) throw new InvalidOperationException($"你的无缝联机版本的配置里没有 {key} 这一项");
            var backup = path + BackupSuffix;
            if (!File.Exists(backup)) File.Copy(path, backup);
            text = line.Replace(text, m => m.Groups[1].Value + " " + value, 1);
            var bytes = new UTF8Encoding(false).GetBytes(text);
            File.WriteAllBytes(path, bom ? new byte[] { 0xef, 0xbb, 0xbf }.Concat(bytes).ToArray() : bytes);
        }

        static string Validate(string key, string value)
        {
            value = (value ?? "").Trim();
            if (key == Password)
            {
                if (value.Length == 0) throw new InvalidOperationException("联机密码不能为空：双方要填同一个密码才能连上");
                if (value.Length > 64 || value.Any(char.IsControl) || value.IndexOfAny(new[] { ';', '#', '=' }) >= 0)
                    throw new InvalidOperationException("联机密码最多 64 个字符，不能含 ; # = 和换行");
                return value;
            }
            if (Switches.Contains(key))
            {
                if (value != "0" && value != "1") throw new InvalidOperationException($"{key} 只能是 0 或 1");
                return value;
            }
            if (Scalings.Contains(key))
            {
                if (!int.TryParse(value, out var percent) || percent < 0 || percent > 1000) throw new InvalidOperationException($"{key} 应为 0–1000 的整数");
                return percent.ToString();
            }
            throw new InvalidOperationException(Forced.Contains(key) ? $"{key} 由爬塔的无缝入口在启动时设定，这里改了也不生效" : $"启动器不管理 {key}，请打开配置文件手动修改");
        }
    }
}
