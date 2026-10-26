using SoulsFormats;
using System.Text.RegularExpressions;
static class AffixTextFix {
    public const int CaptionLines=7;
    static readonly Dictionary<string,string> Labels=new(){["STR"]="力气",["DEX"]="灵巧",["INT"]="智力",["FAI"]="信仰",["ARC"]="感应"};
    static readonly Regex Build=new(@"(?<![A-Za-z])(?:STR|DEX|INT|FAI|ARC)(?:/(?:STR|DEX|INT|FAI|ARC))*(?![A-Za-z])");
    public static string Convert(string text)=>Build.Replace(text,m=>m.Value.Split('/').Length==5?"全属性":string.Join('/',m.Value.Split('/').Select(t=>Labels[t])));
    static bool Variant(int id)=>id>=100000000&&id<115000000||id>=160000000&&id<190000000;
    static bool Advanced(int id)=>id>=160000000&&id<190000000;
    // 原版武器说明最多 7 行，下方是固定位置的战技说明；保留词条信息，背景文字按段落截取
    public static string Fit(string caption) {
        int split=caption.LastIndexOf("\n\n");if(split<0)return caption;
        string lore=caption[..split];var info=caption[(split+2)..].Split('\n');
        int budget=CaptionLines-info.Length-1,used=0;var kept=new List<string>();
        foreach(var paragraph in lore.Split("\n\n")) {
            var lines=paragraph.Split('\n');int need=lines.Length+(kept.Count>0?1:0);
            if(used+need>budget){if(kept.Count==0&&budget>0)kept.Add(string.Join('\n',lines.Take(budget)));break;}
            kept.Add(paragraph);used+=need;
        }
        return kept.Count==0?string.Join('\n',info):string.Join("\n\n",kept)+"\n\n"+string.Join('\n',info);
    }
    static string Expected(int fmg,int id,string text)=>text==null||!Variant(id)?text:fmg==25?Fit(Advanced(id)?Convert(text):text):(fmg is 11 or 21)&&Advanced(id)?Convert(text):text;
    public static void Test(string input) {
        int count=0,trimmed=0;
        foreach(var file in new[]{"affix-details.tsv","affix-v2-details.tsv"})foreach(var p in File.ReadAllLines(Path.Combine(input,file)).Select(l=>l.Split('\t')).Where(p=>long.Parse(p[0])<0x10000000)) {
            var caption=p[4].Replace("\\n","\n");var fitted=Fit(Convert(caption));var info=Convert(caption)[(caption.LastIndexOf("\n\n")+2)..];
            if(fitted.Split('\n').Length>CaptionLines||!fitted.EndsWith(info))throw new Exception($"Caption fit failed: {p[0]}");
            count++;if(fitted.Length<Convert(caption).Length)trimmed++;
            if(p[0]=="179510000")Console.WriteLine(fitted);
        }
        Console.WriteLine($"Checked {count} weapon captions; {trimmed} trimmed to {CaptionLines} lines with affix text kept");
    }
    public const int PoolNameFirst=9610000,BarNameFirst=9700000,BarNameStride=200,BossNameFirst=9600000,BarMarker=9620099;
    // 与 src/boss_affixes.rs 的 bar_name 相同：9700000 + 组合序号 × 200 + Boss 名称序号
    static IEnumerable<(int id,string text)> AffixBarNames(string input,Dictionary<int,string> names) {
        var combos=File.ReadAllLines(Path.Combine(input,"boss-affix-combos.txt")).Select(l=>l.Split('|')).ToArray();
        var majors=File.ReadAllLines(Path.Combine(input,"boss-pool.txt")).Select(l=>l.Split('|')).Where(p=>p[9]=="1").Select(p=>int.Parse(p[2])).Distinct();
        foreach(var index in majors) {
            if(index>=BarNameStride)throw new Exception("Boss name index exceeds affix bar stride");
            var boss=names.GetValueOrDefault(BossNameFirst+index)??throw new Exception($"Missing boss name {BossNameFirst+index}");
            foreach(var combo in combos)yield return (BarNameFirst+int.Parse(combo[0])*BarNameStride+index,$"{combo[1]} {boss}");
        }
    }
    static bool AddMarker(string input,string output) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        if(effects.Rows.Any(r=>r.ID==BarMarker))return false;
        var marker=new PARAM.Row(effects.Rows.Single(r=>r.ID==BossAffixAssets.First+95)){ID=BarMarker,Name="Tower boss affix bar names marker"};
        marker["effectEndurance"].Value=0f;marker["vfxId"].Value=-1;effects.Rows.Add(marker);effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());file.Bytes=effects.Write();
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        if(actual.Rows.Count!=effects.Rows.Count||!actual.Rows.Any(r=>r.ID==BarMarker))throw new Exception("Bar name marker not written");
        return true;
    }
    // 与 native/src/enemies.rs 的 POOL_NAMES 相同：按 enemy-pools.txt 行序去重
    public static List<string> PoolNames(string input) {var names=new List<string>();foreach(var p in File.ReadAllLines(Path.Combine(input,"enemy-pools.txt")).Select(l=>l.Split('|')))if(!names.Contains(p[4]))names.Add(p[4]);return names;}
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var messages=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));var changed=0;var added=0;
        var before=messages.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.Select(e=>(e.ID,e.Text)).ToArray());
        foreach(var file in messages.Files.Where(f=>f.ID is 11 or 21 or 25)) {
            var fmg=FMG.Read(file.Bytes);
            foreach(var entry in fmg.Entries) {var text=Expected(file.ID,entry.ID,entry.Text);if(text!=entry.Text){entry.Text=text;changed++;}}
            file.Bytes=fmg.Write();
        }
        var retired=File.ReadAllLines(Path.Combine(input,"retired-spell-rows.txt")).Where(l=>l.StartsWith("EquipParamGoods|")).Select(l=>int.Parse(l.Split('|')[1])).ToHashSet();var dropped=0;
        foreach(var file in messages.Files.Where(f=>f.ID is 10 or 20 or 24)) {
            var fmg=FMG.Read(file.Bytes);dropped+=fmg.Entries.RemoveAll(e=>retired.Contains(e.ID));file.Bytes=fmg.Write();
        }
        var poolNames=PoolNames(input);var npcFile=messages.Files.First(f=>f.Name.Contains("NpcName"));var npcNames=FMG.Read(npcFile.Bytes);
        for(int i=0;i<poolNames.Count;i++) {
            var existing=npcNames.Entries.FirstOrDefault(e=>e.ID==PoolNameFirst+i);
            if(existing==null){npcNames.Entries.Add(new FMG.Entry(PoolNameFirst+i,poolNames[i]));added++;}
            else if(existing.Text!=poolNames[i])throw new Exception($"Enemy bar name collision: {PoolNameFirst+i}");
        }
        var barNames=AffixBarNames(input,npcNames.Entries.ToDictionary(e=>e.ID,e=>e.Text)).ToList();
        foreach(var (id,text) in barNames) {
            var existing=npcNames.Entries.FirstOrDefault(e=>e.ID==id);
            if(existing==null){npcNames.Entries.Add(new FMG.Entry(id,text));added++;}
            else if(existing.Text!=text)throw new Exception($"Affix bar name collision: {id}");
        }
        npcNames.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));npcFile.Bytes=npcNames.Write();
        bool marker=AddMarker(input,output);
        if(changed==0&&added==0&&dropped==0){Console.WriteLine(marker?"Item text already updated; wrote regulation.bin with the affix bar name marker":"Item text and affix bar name marker already present; nothing written");return;}
        var dir=Path.Combine(output,"msg/zhocn");Directory.CreateDirectory(dir);var path=Path.Combine(dir,"item_dlc02.msgbnd.dcx");messages.Write(path);
        var written=BND4.Read(path).Files;var check=written.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text));
        foreach(var (fmg,entries) in before)foreach(var (id,text) in entries) {
            if((fmg is 10 or 20 or 24)&&retired.Contains(id)){if(check[fmg].ContainsKey(id))throw new Exception($"Retired spell text remains: {fmg}/{id}");continue;}
            if(check[fmg][id]!=((fmg is 11 or 21 or 25)?Expected(fmg,id,text):text))throw new Exception($"Unexpected text change: {fmg}/{id}");
            if(fmg==25&&Variant(id)&&check[fmg][id]?.Split('\n').Length>CaptionLines)throw new Exception($"Weapon caption still exceeds {CaptionLines} lines: {id}");
        }
        var npcCheck=FMG.Read(written.First(f=>f.Name.Contains("NpcName")).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
        for(int i=0;i<poolNames.Count;i++)if(npcCheck.GetValueOrDefault(PoolNameFirst+i)!=poolNames[i])throw new Exception($"Enemy bar name missing: {poolNames[i]}");
        foreach(var (id,text) in barNames)if(npcCheck.GetValueOrDefault(id)!=text)throw new Exception($"Affix bar name missing: {id}");
        var known=before[npcFile.ID].Select(e=>e.ID).Concat(barNames.Select(b=>b.id)).ToHashSet();
        foreach(var (id,_) in npcCheck)if(!known.Contains(id)&&!(id>=PoolNameFirst&&id<PoolNameFirst+poolNames.Count))throw new Exception($"Unexpected NPC name added: {id}");
        foreach(var line in File.ReadAllLines(Path.Combine(input,"affix-v2-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="weapon")) {
            if(check[11].GetValueOrDefault(int.Parse(line[1]))!=line[4])throw new Exception($"Weapon name differs from catalog: {line[1]}");
        }
        Console.WriteLine($"Updated {changed} affix weapon text entries (localized scaling labels, captions fit {CaptionLines} lines) and added {added} bar names (enemies {PoolNameFirst}-{PoolNameFirst+poolNames.Count-1}, {barNames.Count} affix boss titles from {BarNameFirst}){(marker?"; wrote regulation.bin with the affix bar name marker":"")}, removed {dropped} retired spell text entries; all other text unchanged");
    }
}
