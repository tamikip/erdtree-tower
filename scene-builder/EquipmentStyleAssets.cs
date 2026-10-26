using SoulsFormats;
using System.Text.RegularExpressions;

static class EquipmentStyleAssets {
    static readonly Dictionary<int,string> Mount=new() {
        [1]="Dagger",[3]="SwordNormal",[5]="SwordLarge",[7]="SwordGigantic",[9]="SaberNormal",[11]="SaberLarge",
        [13]="katana",[14]="SwordDoubleEdge",[15]="SwordPierce",[16]="RapierHeavy",[17]="AxeNormal",[19]="AxeLarge",
        [21]="HammerNormal",[23]="HammerLarge",[24]="Flail",[25]="SpearNormal",[28]="SpearHeavy",[29]="SpearAxe",
        [31]="Sickle",[35]="Knuckle",[37]="Claw",[39]="Whip",[41]="AxhammerLarge",[50]="BowSmall",[51]="BowNormal",
        [53]="BowLarge",[55]="ClossBow",[56]="Ballista",[57]="Staff",[61]="Talisman",[65]="ShieldSmall",
        [67]="ShieldNormal",[69]="ShieldLarge",[87]="Torch",[88]="HandToHand",[89]="PerfumeBottle",[90]="ThrustingShield",
        [91]="ThrowingWeapon",[92]="ReverseHandSword",[93]="LightGreatsword",[94]="GreatKatana",[95]="BeastClaw"
    };
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    static ulong Mix(ulong n){unchecked{n=(n^(n>>30))*0xbf58476d1ce4e5b9UL;n=(n^(n>>27))*0x94d049bb133111ebUL;return n^(n>>31);}}
    public static string Plain(string text)=>text==null?null:Regex.Replace(text,"</?font(?: color=\"#[0-9A-Fa-f]{6}\")?>","");
    static string Color(string text,string hex)=>string.IsNullOrWhiteSpace(text)?text:$"<font color=\"#{hex}\">{Plain(text)}</font>";
    public static bool SameName(string actual,string canonical)=>Plain(actual)==canonical;
    static bool Compatible(PARAM.Row weapon,PARAM.Row gem)=>I(weapon,"gemMountType")==2
        &&Mount.TryGetValue(I(weapon,"wepType"),out var field)&&I(gem,"canMountWep_"+field)!=0
        &&(I(weapon,"restrictSpecialSwordArt")==0||I(gem,"isSpecialSwordArt")==0);
    public static void Verify(Func<string,PARAM> read,string input) {
        var weapons=read("EquipParamWeapon").Rows.ToDictionary(r=>r.ID);var gems=read("EquipParamGem").Rows.ToDictionary(r=>r.ID);
        var sold=File.ReadAllLines(Path.Combine(input,"shop-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="4").Select(p=>int.Parse(p[1])).ToHashSet();
        var manifest=File.ReadAllLines(Path.Combine(input,"random-ashes.tsv")).Skip(1).Select(l=>l.Split('\t')).ToDictionary(p=>int.Parse(p[0]));
        var variants=new[]{"affix-catalog.txt","affix-v2-catalog.txt"}.SelectMany(n=>File.ReadAllLines(Path.Combine(input,n))).Select(l=>l.Split('|')).Where(p=>p[0]=="weapon").ToArray();
        if(manifest.Count!=variants.Length)throw new Exception("Random ash coverage mismatch");
        foreach(var p in variants) {
            int id=int.Parse(p[1]),baseId=int.Parse(p[2]);var m=manifest[id];int gem=int.Parse(m[2]),art=int.Parse(m[3]);
            if(int.Parse(m[1])!=baseId)throw new Exception("Random ash base mismatch");
            if(gem>=0&&(!sold.Contains(gem)||!Compatible(weapons[baseId],gems[gem])||I(gems[gem],"swordArtsParamId")!=art))throw new Exception("Incompatible random ash");
            if(gem<0&&I(weapons[baseId],"swordArtsParamId")!=art)throw new Exception("Unmodifiable weapon skill changed");
            foreach(var row in weapons.Values.Where(r=>r.ID>=id&&r.ID<id+10000))if(I(row,"swordArtsParamId")!=art)throw new Exception("Affinity variant skill mismatch");
        }
        Console.WriteLine($"Verified random ash compatibility and affinity rows for {manifest.Count} weapon variants");
    }

    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging directory required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        PARAM Read(string name) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;
        }
        var weapons=Read("EquipParamWeapon");var originals=weapons.Rows.ToDictionary(r=>r.ID,r=>new PARAM.Row(r));
        var rows=weapons.Rows.ToDictionary(r=>r.ID);var arts=Read("SwordArtsParam").Rows.ToDictionary(r=>r.ID);
        var sold=File.ReadAllLines(Path.Combine(input,"shop-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="4").Select(p=>int.Parse(p[1])).ToHashSet();
        var gems=Read("EquipParamGem").Rows.Where(r=>sold.Contains(r.ID)&&arts.ContainsKey(I(r,"swordArtsParamId"))).OrderBy(r=>r.ID).ToArray();
        var messages=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));
        var nativeText=ItemTextTables.Read(messages);
        string Text(string table,int id)=>Plain(nativeText[table].Entries.FirstOrDefault(e=>e.ID==id)?.Text??"");
        var variants=new[]{"affix-catalog.txt","affix-v2-catalog.txt"}.SelectMany(n=>File.ReadAllLines(Path.Combine(input,n))).Select(l=>l.Split('|')).ToArray();
        var manifest=new List<string>{"WeaponId\tBaseId\tGemId\tSwordArtId\tName"};
        var skillText=new Dictionary<uint,string>();var expected=new Dictionary<int,int>();int changed=0,randomized=0;
        foreach(var variant in variants.Where(v=>v[0]=="weapon")) {
            int id=int.Parse(variant[1]),baseId=int.Parse(variant[2]);var source=rows[baseId];int original=I(source,"swordArtsParamId");
            var pool=gems.Where(g=>Compatible(source,g)).ToArray();
            var alternatives=pool.Where(g=>I(g,"swordArtsParamId")!=original).ToArray();if(alternatives.Length>0)pool=alternatives;
            var gem=pool.Length==0?null:pool[(int)(Mix((ulong)id^0x4153482026UL)%(ulong)pool.Length)];
            int art=gem==null?original:I(gem,"swordArtsParamId");if(gem!=null)randomized++;
            int textId=I(arts[art],"textId");var name=Text("ArtsName",textId);var caption=Text("ArtsCaption",textId);
            if(gem!=null&&(string.IsNullOrWhiteSpace(name)||string.IsNullOrWhiteSpace(caption)))throw new Exception($"Skill text missing: {art}");
            skillText[(uint)id]=(name+"\n"+caption).Replace("\r","").Replace("\n","\\n").Replace("\t"," ");
            manifest.Add($"{id}\t{baseId}\t{gem?.ID??-1}\t{art}\t{name}");
            foreach(var row in weapons.Rows.Where(r=>r.ID>=id&&r.ID<id+10000)) {
                if(I(row,"swordArtsParamId")!=art)changed++;
                row["swordArtsParamId"].Value=Convert.ChangeType(art,row["swordArtsParamId"].Value.GetType());expected[row.ID]=art;
            }
        }
        foreach(var row in weapons.Rows)foreach(var cell in row.Cells) {
            if(cell.Def.InternalName=="swordArtsParamId"&&expected.ContainsKey(row.ID))continue;
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,originals[row.ID][cell.Def.InternalName].Value))throw new Exception("Unrelated weapon field changed");
        }
        var weaponFile=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamWeapon");weaponFile.Bytes=weapons.Write();
        Directory.CreateDirectory(output);var regulation=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(regulation,reg);
        if(new FileInfo(regulation).Length>4799*1024)throw new Exception("Regulation exceeds game size limit");
        var restored=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(regulation);
        foreach(var file in reg.Files)if(!file.Bytes.SequenceEqual(restored.Files.Single(f=>f.Name==file.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        File.WriteAllLines(Path.Combine(output,"random-ashes.tsv"),manifest);
        foreach(var name in new[]{"affix-details.tsv","affix-v2-details.tsv"}) {
            var details=File.ReadAllLines(Path.Combine(input,name)).Select(l=>l.Split('\t')).ToArray();
            foreach(var p in details)if(skillText.TryGetValue(uint.Parse(p[0]),out var skill))p[6]=skill;
            File.WriteAllLines(Path.Combine(output,name),details.Select(p=>string.Join('\t',p)));
        }
        var colors=File.ReadAllLines(Path.Combine(input,"equipment-colors.txt")).Select(l=>l.Split('|')).ToDictionary(p=>int.Parse(p[0]),p=>p[1]);
        if(colors.Count!=4||colors.Values.Any(v=>!Regex.IsMatch(v,"^[0-9A-F]{6}$")))throw new Exception("Invalid rarity palette");
        var rarity=File.ReadAllLines(Path.Combine(input,"reward-rarities.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]),p=>Math.Clamp(int.Parse(p[1]),0,3));
        var named=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0] is "weapon" or "armor").ToArray();
        var tiers=named.ToDictionary(p=>(p[0],int.Parse(p[1])),p=>rarity[(p[0]=="weapon"?0u:0x10000000u)|uint.Parse(p[1])]);
        var affixed=new HashSet<(string,int)>();
        foreach(var p in variants){var key=(p[0],int.Parse(p[1]));tiers[key]=int.Parse(p[3]);affixed.Add(key);}
        int colored=0;
        foreach(var file in messages.Files) {
            string table=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];
            if(!new[]{"WeaponName","WeaponInfo","WeaponCaption","ProtectorName","ProtectorInfo","ProtectorCaption"}.Contains(table))continue;
            string kind=table.StartsWith("Weapon")?"weapon":"armor";var fmg=FMG.Read(file.Bytes);
            foreach(var entry in fmg.Entries) {
                if(!tiers.TryGetValue((kind,entry.ID),out int tier)||string.IsNullOrWhiteSpace(entry.Text))continue;
                var plain=Plain(entry.Text);var before=plain;
                if(table.EndsWith("Name")) {
                    entry.Text=Color(plain,colors[tier]);
                } else if(table.EndsWith("Caption")) {
                    if(!affixed.Contains((kind,entry.ID)))continue;
                    int split=plain.LastIndexOf("\n\n");
                    entry.Text=split<0?Color(plain,colors[tier]):plain[..(split+2)]+Color(plain[(split+2)..],colors[tier]);
                } else entry.Text=Color(plain,colors[tier]);
                if(Plain(entry.Text)!=before)throw new Exception("Color markup changed item text");colored++;
            }
            file.Bytes=fmg.Write();
        }
        var textPath=Path.Combine(output,"msg/zhocn/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(textPath)!);messages.Write(textPath);
        var written=BND4.Read(textPath);
        foreach(var file in messages.Files)if(!file.Bytes.SequenceEqual(written.Files.Single(f=>f.ID==file.ID).Bytes))throw new Exception("Text roundtrip mismatch");
        File.WriteAllText(Path.Combine(output,"equipment-style-build.json"),System.Text.Json.JsonSerializer.Serialize(new{randomizedWeapons=randomized,changedWeaponRows=changed,coloredTextEntries=colored,compatibleNativeAshesOnly=true,unrelatedWeaponFieldsPreserved=true,plainTextPreserved=true}));
        Console.WriteLine($"Verified {randomized} compatible randomized weapon variants, {changed} changed weapon rows, {colored} colored text entries; native and DLC ashes included");
    }
}
