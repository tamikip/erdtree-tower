using SoulsFormats;
using System.Text.Json;
using System.Globalization;

static class SpellAffixAssets {
    static readonly string[] Names=["强效","节能","轻盈","破势","蓄能","绵延","慈悲","蚀骨"];
    static readonly string[] English=["Potent","Efficient","Lightweight","Staggering","Charged","Lasting","Merciful","Corrosive"];
    static readonly int[][] Values=[[10,18,28,40],[10,18,28,40],[15,25,35,50],[15,25,40,60],[15,25,40,55],[20,35,50,75],[15,25,40,60],[10,18,28,40]];
    static readonly string[] Damage=["atkPhys","atkMag","atkFire","atkThun","atkDark"];
    static readonly string[] Status=["poizonAttackPower","diseaseAttackPower","bloodAttackPower","freezeAttackPower","sleepAttackPower","madnessAttackPower","curseAttackPower"];
    static int I(PARAM.Row r,string k)=>Convert.ToInt32(r[k].Value);
    static double F(PARAM.Row r,string k)=>Convert.ToDouble(r[k].Value);
    static void Set(PARAM.Row r,string k,double v)=>r[k].Value=Convert.ChangeType(v,r[k].Value.GetType(),CultureInfo.InvariantCulture);
    static IEnumerable<(string field,string table,int id)> References(string table,PARAM.Row r) {
        if(table=="Magic") {
            for(int i=1;i<=10;i++)if(I(r,$"refId{i}")>0)yield return ($"refId{i}",I(r,$"refCategory{i}") switch {0=>"AtkParam_Pc",1=>"Bullet",2=>"SpEffectParam",_=>throw new Exception("Unknown spell reference")},I(r,$"refId{i}"));
        }else if(table=="Bullet") {
            foreach(var field in new[]{"atkId_Bullet","HitBulletID","intervalCreateBulletId","spEffectIDForShooter","spEffectId0","spEffectId1","spEffectId2","spEffectId3","spEffectId4"})
                if(I(r,field)>0)yield return (field,field=="atkId_Bullet"?"AtkParam_Pc":field is "HitBulletID" or "intervalCreateBulletId"?"Bullet":"SpEffectParam",I(r,field));
        }else if(table=="AtkParam_Pc") {
            for(int i=0;i<5;i++)if(I(r,$"spEffectId{i}")>0)yield return ($"spEffectId{i}","SpEffectParam",I(r,$"spEffectId{i}"));
        }else if(table=="SpEffectParam") {
            foreach(var field in new[]{"replaceSpEffectId","cycleOccurrenceSpEffectId","atkOccurrenceSpEffectId"})if(I(r,field)>0)yield return (field,table,I(r,field));
        }
    }
    static IEnumerable<string> ChangedFields(string table,PARAM.Row r,int kind) {
        if(table=="AtkParam_Pc") {
            if(kind is 0 or 4)foreach(var f in Damage)if(F(r,f)>0)yield return f;
            if(kind==3&&F(r,"atkSuperArmor")>0)yield return "atkSuperArmor";
        }
        if(table=="SpEffectParam") {
            if(kind==5&&F(r,"effectEndurance")>=1)yield return "effectEndurance";
            if(kind==6&&F(r,"effectEndurance")==0)foreach(var f in new[]{"changeHpPoint","changeHpRate"})if(F(r,f)<0)yield return f;
            if(kind==7)foreach(var f in Status)if(r.Cells.Any(c=>c.Def.InternalName==f)&&F(r,f)>0)yield return f;
        }
    }
    public static void Build(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging directory required");
        if(File.Exists(Path.Combine(input,"spell-affix-catalog.txt")))throw new Exception("Spell variants already generated; use the preserved pre-affix inputs to rebuild without changing IDs");
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var tables=new Dictionary<string,PARAM>();
        foreach(var name in new[]{"EquipParamGoods","Magic","Bullet","AtkParam_Pc","SpEffectParam"}) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{Defs.GetValueOrDefault(name,name)}.xml"));tables[name]=p;
        }
        var original=tables.ToDictionary(p=>p.Key,p=>p.Value.Rows.ToDictionary(r=>r.ID));
        var occupied=tables.ToDictionary(p=>p.Key,p=>p.Value.Rows.Select(r=>r.ID).ToHashSet());
        var next=tables.Keys.ToDictionary(k=>k,k=>80000000);
        int Allocate(string table) {while(occupied[table].Contains(next[table]))next[table]++;int id=next[table]++;occupied[table].Add(id);return id;}
        var catalog=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0] is "sorcery" or "incantation").OrderBy(p=>int.Parse(p[1])).ToArray();
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var manifest=new List<string>();var detailLines=new List<string>();var rarityLines=new List<string>();
        var variants=new List<(int id,int basis,int kind,int tier)>();int nextMagic=20000;
        var bulletOrigins=new SortedDictionary<int,int>();
        var reservedText=new HashSet<int>();
        foreach(var lang in new[]{"zhocn","engus"})foreach(var file in BND4.Read(Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx")).Files)
            if(Path.GetFileNameWithoutExtension(file.Name).StartsWith("Goods"))reservedText.UnionWith(FMG.Read(file.Bytes).Entries.Select(e=>e.ID));
        foreach(var entry in catalog) {
            int basis=int.Parse(entry[1]);var magic=original["Magic"][basis];var goods=original["EquipParamGoods"][basis];
            if(I(magic,"replaceMagicId")>0)throw new Exception($"Spell replacement needs explicit handling: {basis}");
            var refs=References("Magic",magic).ToArray();
            var graph=new Dictionary<(string table,int id),PARAM.Row>();
            bool complete=true;
            void Visit(string table,int id) {
                var key=(table,id);if(graph.ContainsKey(key))return;
                if(!original[table].TryGetValue(id,out var row)){complete=false;return;}
                graph[key]=row;foreach(var child in References(table,row))Visit(child.table,child.id);
            }
            foreach(var r in refs)Visit(r.table,r.id);
            if(!complete)Console.WriteLine($"Spell {basis}: unresolved original references; only cost affixes enabled");
            bool Effective(string table,PARAM.Row row,int kind)=>ChangedFields(table,row,kind).Any(f=>
                !Equals(row[f].Value,Convert.ChangeType(F(row,f)*(1+Values[kind][0]/100.0),row[f].Value.GetType(),CultureInfo.InvariantCulture)));
            bool Has(int kind)=>graph.Any(p=>Effective(p.Key.table,p.Value,kind));
            bool Raw(int kind)=>graph.Any(p=>ChangedFields(p.Key.table,p.Value,kind).Any());
            for(int kind=0;kind<8;kind++) {
                if(!complete&&kind is not 1 and not 2)continue;
                bool eligible=kind switch {1=>I(magic,"mp")>1||I(magic,"mp_charge")>1,2=>I(magic,"stamina")>1||I(magic,"stamina_charge")>1,
                    4=>refs.Length==2&&I(magic,"consumeType1")==0&&I(magic,"consumeType2")==1&&Has(0),5=>!Raw(0)&&!Raw(7)&&Has(5),_=>Has(kind)};
                if(!eligible)continue;
                IEnumerable<(string field,string table,int id)> Links(string table,PARAM.Row row)=>References(table,row).Where(r=>kind!=7||r.field!="spEffectIDForShooter");
                var applicable=graph.Keys.ToHashSet();
                if(kind is 4 or 7) {
                    applicable.Clear();
                    void Reach(string table,int id) {if(!applicable.Add((table,id)))return;foreach(var child in Links(table,graph[(table,id)]))Reach(child.table,child.id);}
                    if(kind==4)Reach(refs[1].table,refs[1].id);
                    else foreach(var r in refs.Where(r=>r.table!="SpEffectParam"))Reach(r.table,r.id);
                }
                var affected=graph.Where(p=>applicable.Contains(p.Key)&&Effective(p.Key.table,p.Value,kind)).Select(p=>p.Key).ToHashSet();
                if(kind is not 1 and not 2&&affected.Count==0)continue;
                bool grew;do {grew=false;foreach(var p in graph)if(applicable.Contains(p.Key)&&Links(p.Key.table,p.Value).Any(r=>affected.Contains((r.table,r.id))))grew|=affected.Add(p.Key);}while(grew);
                for(int tier=0;tier<4;tier++) {
                    while(occupied["Magic"].Contains(nextMagic)||occupied["EquipParamGoods"].Contains(nextMagic)||reservedText.Contains(nextMagic))nextMagic++;
                    if(nextMagic>32767)throw new Exception("Spell variant ID exceeds signed short range");
                    int id=nextMagic++;occupied["Magic"].Add(id);occupied["EquipParamGoods"].Add(id);
                    var m=new PARAM.Row(magic){ID=id};var g=new PARAM.Row(goods){ID=id};
                    var ids=affected.OrderBy(k=>k.table).ThenBy(k=>k.id).ToDictionary(k=>k,k=>Allocate(k.table));
                    double factor=1+Values[kind][tier]/100.0;
                    foreach(var key in ids.Keys) {
                        if(key.table=="Bullet")bulletOrigins[ids[key]]=key.id;
                        var row=new PARAM.Row(graph[key]){ID=ids[key]};
                        foreach(var field in ChangedFields(key.table,row,kind))Set(row,field,F(row,field)*factor);
                        foreach(var r in Links(key.table,graph[key]))if(ids.TryGetValue((r.table,r.id),out int target))Set(row,r.field,target);
                        tables[key.table].Rows.Add(row);
                    }
                    foreach(var r in refs)if((kind!=4||r.field==refs[1].field)&&(kind!=7||r.table!="SpEffectParam")&&ids.TryGetValue((r.table,r.id),out int target))Set(m,r.field,target);
                    if(kind is 1 or 2)foreach(var field in kind==1?new[]{"mp","mp_charge"}:new[]{"stamina","stamina_charge"})if(I(m,field)>0)Set(m,field,Math.Max(1,Math.Floor(I(m,field)*(1-Values[kind][tier]/100.0))));
                    Set(g,"rarity",tier);tables["Magic"].Rows.Add(m);tables["EquipParamGoods"].Rows.Add(g);
                    string name=$"{new[]{"普通","稀有","史诗","传说"}[tier]}·{Names[kind]}·{entry[2]}";
                    manifest.Add($"{entry[0]}|{id}|{basis}|{tier}|{name}|{kind}");
                    var d=(string[])details[0x40000000u|(uint)basis].Clone();d[0]=(0x40000000u|(uint)id).ToString();d[2]=name;detailLines.Add(string.Join('\t',d));
                    rarityLines.Add($"{0x40000000u|(uint)id}\t{tier}");variants.Add((id,basis,kind,tier));
                }
            }
        }
        Directory.CreateDirectory(output);
        foreach(var (name,p) in tables) {p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes=p.Write();}
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),reg);
        foreach(var lang in new[]{"zhocn","engus"}) {
            var msg=BND4.Read(Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx"));var merged=ItemTextTables.Read(msg);bool en=lang=="engus";
            foreach(var file in msg.Files) {
                var key=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];if(key is not "GoodsName" and not "GoodsInfo" and not "GoodsCaption")continue;
                var fmg=FMG.Read(file.Bytes);var existing=fmg.Entries.Select(e=>e.ID).ToHashSet();
                foreach(var v in variants) {
                    if(existing.Contains(v.id))throw new Exception($"Spell text collision {lang}:{v.id}");
                    string baseName=merged["GoodsName"].Entries.Single(e=>e.ID==v.basis).Text;
                    string text=key=="GoodsName"?(en?$"{new[]{"Common","Rare","Epic","Legendary"}[v.tier]} {English[v.kind]} {baseName}":$"{new[]{"普通","稀有","史诗","传说"}[v.tier]}·{Names[v.kind]}·{baseName}")
                        :(merged[key].Entries.FirstOrDefault(e=>e.ID==v.basis)?.Text??"");
                    fmg.Entries.Add(new FMG.Entry(v.id,text));
                }
                fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=fmg.Write();
            }
            Directory.CreateDirectory(Path.Combine(output,$"msg/{lang}"));msg.Write(Path.Combine(output,$"msg/{lang}/item_dlc02.msgbnd.dcx"));
        }
        File.WriteAllLines(Path.Combine(output,"spell-affix-catalog.txt"),manifest);File.WriteAllLines(Path.Combine(output,"spell-affix-details.tsv"),detailLines);File.WriteAllLines(Path.Combine(output,"spell-affix-rarities.tsv"),rarityLines);
        File.WriteAllLines(Path.Combine(output,"spell-affix-bullets.tsv"),bulletOrigins.Select(p=>$"{p.Key}\t{p.Value}"));
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        foreach(var file in check.Files) {
            string name=Path.GetFileNameWithoutExtension(file.Name.Replace('\\','/'));
            if(!tables.ContainsKey(name)){if(!file.Bytes.SequenceEqual(before[file.Name]))throw new Exception($"Unrelated table changed: {name}");continue;}
            var p=PARAM.Read(file.Bytes);p.ApplyParamdef(tables[name].AppliedParamdef);var rows=p.Rows.ToDictionary(r=>r.ID);
            foreach(var old in original[name].Values)foreach(var c in old.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,rows[old.ID][c.Def.InternalName].Value))throw new Exception($"Original row changed: {name}:{old.ID}");
            if(p.Rows.Count!=tables[name].Rows.Count)throw new Exception($"Generated row count mismatch: {name}");
        }
        Console.WriteLine($"Built {variants.Count} spell affixes from {catalog.Length} spells; all original parameter rows preserved");
        foreach(var group in variants.GroupBy(v=>v.kind))Console.WriteLine($"{Names[group.Key]}: {group.Count()/4} spells");
    }
    public static void OriginalDescriptions(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging directory required");
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var bases=File.ReadAllLines(Path.Combine(input,"spell-affix-catalog.txt")).Select(l=>l.Split('|')).ToDictionary(p=>int.Parse(p[1]),p=>int.Parse(p[2]));
        foreach(var lang in new[]{"zhocn","engus"}) {
            var msg=BND4.Read(Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx"));var merged=ItemTextTables.Read(msg);
            var before=msg.Files.ToDictionary(f=>f.ID,f=>f.Bytes.ToArray());
            foreach(var file in msg.Files) {
                var key=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];if(key is not "GoodsInfo" and not "GoodsCaption")continue;
                var fmg=FMG.Read(file.Bytes);var source=merged[key].Entries.ToDictionary(e=>e.ID,e=>e.Text);
                foreach(var entry in fmg.Entries)if(bases.TryGetValue(entry.ID,out int basis))entry.Text=source.GetValueOrDefault(basis,"");
                file.Bytes=fmg.Write();
            }
            var path=Path.Combine(output,$"msg/{lang}/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(path));msg.Write(path);
            var check=BND4.Read(path);int count=0;
            foreach(var file in check.Files) {
                var key=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];
                if(key is not "GoodsInfo" and not "GoodsCaption"){if(!file.Bytes.SequenceEqual(before[file.ID]))throw new Exception("Unrelated text changed");continue;}
                var old=FMG.Read(before[file.ID]).Entries.ToDictionary(e=>e.ID,e=>e.Text);var source=merged[key].Entries.ToDictionary(e=>e.ID,e=>e.Text);
                foreach(var entry in FMG.Read(file.Bytes).Entries) {
                    string expected=bases.TryGetValue(entry.ID,out int basis)?source.GetValueOrDefault(basis,""):old[entry.ID];
                    if(entry.Text!=expected)throw new Exception($"Spell description mismatch: {lang}:{entry.ID}");
                    if(bases.ContainsKey(entry.ID))count++;
                }
            }
            if(count<2*bases.Count)throw new Exception("Missing spell descriptions");
            Console.WriteLine($"{lang}: verified {count} original spell descriptions; names and unrelated text preserved");
        }
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var lines=File.ReadAllLines(Path.Combine(input,"spell-affix-details.tsv")).Select(l=>{
            var p=l.Split('\t');var source=details[0x40000000u|(uint)bases[(int)(uint.Parse(p[0])&0x0fffffff)]];
            p[3]=source[3];p[4]=source[4];return string.Join('\t',p);
        });
        File.WriteAllLines(Path.Combine(output,"spell-affix-details.tsv"),lines);
    }
    static readonly Dictionary<string,string> Defs=new(){["Magic"]="MagicParam",["Bullet"]="BulletParam",["AtkParam_Pc"]="AtkParam",["SpEffectParam"]="SpEffect"};
    public static void Inspect(string input,string output) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        PARAM Read(string name) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{Defs.GetValueOrDefault(name,name)}.xml"));return p;
        }
        var tables=new[]{"EquipParamGoods","Magic","Bullet","AtkParam_Pc","SpEffectParam"}.ToDictionary(n=>n,Read);
        Directory.CreateDirectory(output);
        foreach(var (name,p) in tables)File.WriteAllText(Path.Combine(output,name+".json"),JsonSerializer.Serialize(p.Rows.ToDictionary(r=>r.ID,r=>r.Cells.Where(c=>c.Value is not byte[]).ToDictionary(c=>c.Def.InternalName,c=>c.Value))));
        Console.WriteLine("Exported spell reference tables for offline inspection");
    }
}
