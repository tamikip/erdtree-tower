using SoulsFormats;
using System.Text.Json;

static class TalismanAssets {
    record Definition(int Id,int Effect,int Source,int Tier,string Kind,string Name,string Info);
    static int I(PARAM.Row r,string f)=>Convert.ToInt32(r[f].Value);
    static void Set(PARAM.Row r,string f,int v)=>r[f].Value=Convert.ChangeType(v,r[f].Value.GetType());
    static string FileName(BinderFile f)=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'));
    static string Def(string table)=>table switch {"SpEffectParam"=>"SpEffect","SpEffectVfxParam"=>"SpEffectVfx","BehaviorParam_PC"=>"BehaviorParam","Bullet"=>"BulletParam","AtkParam_Pc"=>"AtkParam",_=>table};
    static PARAM Read(BND4 reg,string table){var p=PARAM.Read(reg.Files.Single(f=>FileName(f)==table).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{Def(table)}.xml"));return p;}
    static int Mapped(string table,int id)=>checked(id+(table=="SpEffectParam"?50000000:table=="SpEffectVfxParam"?10000000:100000000));
    static IEnumerable<(string Field,string Table)> References(string table,PARAM.Row row) {
        if(table=="SpEffectParam") {
            foreach(var field in new[]{"replaceSpEffectId","cycleOccurrenceSpEffectId","atkOccurrenceSpEffectId","accumuOverFireId","accumuUnderFireId","applyIdOnGetSoul","spiritDeathSpEffectId"})yield return(field,"SpEffectParam");
            yield return("behaviorId","BehaviorParam_PC");
            foreach(var field in new[]{"vfxId","vfxId1","vfxId2","vfxId3","vfxId4","vfxId5","vfxId6","vfxId7"})yield return(field,"SpEffectVfxParam");
        } else if(table=="BehaviorParam_PC") {
            yield return("refId",I(row,"refType") switch {0=>"AtkParam_Pc",1=>"Bullet",2=>"SpEffectParam",_=>throw new Exception("Unknown behavior reference type")});
        } else if(table=="Bullet") {
            foreach(var field in new[]{"HitBulletID","intervalCreateBulletId"})yield return(field,"Bullet");
            foreach(var field in new[]{"spEffectIDForShooter","spEffectId0","spEffectId1","spEffectId2","spEffectId3","spEffectId4"})yield return(field,"SpEffectParam");
            yield return("atkId_Bullet","AtkParam_Pc");
        } else if(table=="AtkParam_Pc") {
            for(int i=0;i<5;i++)yield return($"spEffectId{i}","SpEffectParam");
        }
    }
    static Definition[] Definitions(string input)=>File.ReadAllLines(Path.Combine(input,"talisman-definitions.tsv")).Select(l=>l.Split('\t')).Select(p=>new Definition(int.Parse(p[0]),int.Parse(p[1]),int.Parse(p[2]),int.Parse(p[3]),p[4],p[5],p[6])).ToArray();
    static string Caption(Definition d)=>d.Info+"\n\n装备后生效，占用护符栏位。";
    /// 护符名称（13）、效果说明（23）与详细说明（27），按品质着色
    static void WriteText(Dictionary<int,FMG> textTables,Definition d,Dictionary<int,string> colors) {
        string Color(string text)=>$"<font color=\"#{colors[d.Tier]}\">{text}</font>";
        foreach(var (table,text) in new[]{(13,d.Name),(23,d.Info),(27,Caption(d))}) {
            var entries=textTables[table].Entries;entries.RemoveAll(e=>e.ID==d.Id);entries.Add(new FMG.Entry(d.Id,Color(text)));
        }
    }
    static Dictionary<int,string> Colors(string input)=>File.ReadAllLines(Path.Combine(input,"equipment-colors.txt")).Select(l=>l.Split('|')).ToDictionary(p=>int.Parse(p[0]),p=>p[1]);
    /// talisman-text-patch：只按 talisman-definitions.tsv 重写护符的名称与说明（不需要原包 regulation），其余文本不变
    public static void PatchText(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        const string name="msg/zhocn/item_dlc02.msgbnd.dcx";
        var messages=BND4.Read(Path.Combine(input,name));var colors=Colors(input);var definitions=Definitions(input);
        var textTables=new[]{13,23,27}.ToDictionary(id=>id,id=>FMG.Read(messages.Files.Single(f=>f.ID==id).Bytes));
        var before=textTables.ToDictionary(p=>p.Key,p=>p.Value.Entries.ToDictionary(e=>e.ID,e=>e.Text));
        foreach(var d in definitions) {
            if(!before[13].ContainsKey(d.Id))throw new Exception($"Talisman text missing, run talisman-affix-patch first: {d.Id}");
            WriteText(textTables,d,colors);
        }
        foreach(var (id,fmg) in textTables){fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));messages.Files.Single(f=>f.ID==id).Bytes=fmg.Write();}
        var original=BND4.Read(Path.Combine(input,name));
        var path=Path.Combine(output,name);Directory.CreateDirectory(Path.GetDirectoryName(path)!);messages.Write(path);
        var check=BND4.Read(path);var ids=definitions.Select(d=>d.Id).ToHashSet();int changed=0;
        foreach(var f in original.Files.Where(f=>!textTables.ContainsKey(f.ID)))if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.ID==f.ID).Bytes))throw new Exception("Unrelated message file changed");
        foreach(var (id,_) in textTables) {
            var actual=FMG.Read(check.Files.Single(f=>f.ID==id).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
            foreach(var (entry,text) in before[id])if(!ids.Contains(entry)&&actual[entry]!=text)throw new Exception($"Unrelated text changed: {id}/{entry}");
            changed+=ids.Count(entry=>actual[entry]!=before[id][entry]);
        }
        Console.WriteLine($"Talisman texts rewritten for {definitions.Length} talismans ({changed} entries changed); other texts preserved");
    }
    public static void Build(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var sourcePath=Environment.GetEnvironmentVariable("ERDTREE_RUBY_SOURCE")??throw new Exception("Set ERDTREE_RUBY_SOURCE to the supplied Ruby regulation.bin");
        var source=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(sourcePath);
        var target=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=target.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var definitions=Definitions(input);
        var tables=new[]{"SpEffectParam","BehaviorParam_PC","Bullet","AtkParam_Pc","SpEffectVfxParam"};
        var sources=tables.ToDictionary(t=>t,t=>Read(source,t).Rows.ToDictionary(r=>r.ID));
        var targets=tables.ToDictionary(t=>t,t=>Read(target,t));
        var required=new HashSet<(string Table,int Id)>();var queue=new Queue<(string,int)>();
        foreach(var d in definitions)for(int n=0;n<10;n++)if(sources["SpEffectParam"].ContainsKey(d.Effect+n))queue.Enqueue(("SpEffectParam",d.Effect+n));
        while(queue.TryDequeue(out var key)) {
            if(!required.Add(key))continue;
            if(!sources[key.Item1].TryGetValue(key.Item2,out var row))throw new Exception($"Missing source dependency {key}");
            foreach(var (field,table) in References(key.Item1,row))if(I(row,field)>0&&(table!="SpEffectVfxParam"||sources[table].ContainsKey(I(row,field))))queue.Enqueue((table,I(row,field)));
        }
        if(required.Any(k=>k.Table=="SpEffectParam"&&k.Id>=7005660&&k.Id<7005700))throw new Exception("Unverified critical-hit effects must not be imported");
        foreach(var (table,id) in required.OrderBy(k=>k.Table).ThenBy(k=>k.Id)) {
            var row=new PARAM.Row(sources[table][id]){ID=Mapped(table,id),Name=$"Tower talisman {table}:{id}"};
            foreach(var (field,refTable) in References(table,row))if(I(row,field)>0&&required.Contains((refTable,I(row,field))))Set(row,field,Mapped(refTable,I(row,field)));
            var existing=targets[table].Rows.SingleOrDefault(r=>r.ID==row.ID);
            if(existing!=null&&existing.Name!=row.Name)throw new Exception($"Parameter collision {table}:{row.ID}");
            if(existing!=null)targets[table].Rows.Remove(existing);targets[table].Rows.Add(row);
        }
        var sourceItems=Read(source,"EquipParamAccessory").Rows.ToDictionary(r=>r.ID);var accessories=Read(target,"EquipParamAccessory");
        var kinds=definitions.Select(d=>d.Kind).Distinct().Order().Select((name,i)=>(name,i)).ToDictionary(p=>p.name,p=>p.i);
        var messages=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));var colors=Colors(input);
        var textTables=new[]{13,23,27}.ToDictionary(id=>id,id=>FMG.Read(messages.Files.Single(f=>f.ID==id).Bytes));
        var catalog=new List<string>();var details=new List<string>();var rarities=new List<string>();var manifest=new List<object>();
        var redrawn=TalismanIconAssets.Icons(input);
        foreach(var d in definitions) {
            var row=new PARAM.Row(sourceItems[d.Source]){ID=d.Id,Name=$"Tower talisman accessory:{d.Effect}"};
            if(I(row,"refId")!=d.Effect)throw new Exception("Source accessory reference mismatch");
            Set(row,"refId",Mapped("SpEffectParam",d.Effect));Set(row,"rarity",d.Tier);Set(row,"sortId",950000+d.Id-960100);
            bool onHit=I(sources["SpEffectParam"][d.Effect],"stateInfo") is 303 or 304 or 305 or 306;
            int group=onHit?24900+d.Tier:25000+kinds[d.Kind]*4+(d.Info.Contains("同类效果不能重复")?0:d.Tier);
            Set(row,"accessoryGroup",group);Set(row,"sellValue",100*(d.Tier+1));
            if(redrawn!=null)Set(row,"iconId",redrawn[d.Id]);
            int icon=I(row,"iconId");if(!File.Exists(Path.Combine(input,"reward-icons",icon+".rgba"))){icon=18000;Set(row,"iconId",icon);}
            if(!File.Exists(Path.Combine(input,"reward-icons",icon+".rgba")))throw new Exception("Talisman icon missing");
            if(accessories.Rows.Any(r=>r.ID!=d.Id&&I(r,"accessoryGroup")==group&&!definitions.Any(x=>x.Id==r.ID)))throw new Exception("Accessory group collision");
            var old=accessories.Rows.SingleOrDefault(r=>r.ID==d.Id);if(old!=null&&old.Name!=row.Name)throw new Exception("Accessory ID collision");
            if(old!=null)accessories.Rows.Remove(old);accessories.Rows.Add(row);
            string caption=Caption(d);WriteText(textTables,d,colors);
            uint full=0x20000000u|(uint)d.Id;
            catalog.Add($"talisman|{d.Id}|{d.Name}");
            details.Add($"{full}\t{icon}\t{d.Name}\t{d.Info}\t{caption.Replace("\n","\\n")}\t重量：{row["weight"].Value}\t");
            rarities.Add($"{full}\t{d.Tier}");
            manifest.Add(new{id=d.Id,sourceEffect=d.Effect,effect=Mapped("SpEffectParam",d.Effect),kind=d.Kind,tier=d.Tier,onHit,group});
        }
        targets["EquipParamAccessory"]=accessories;
        foreach(var (name,param) in targets){param.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));target.Files.Single(f=>FileName(f)==name).Bytes=param.Write();}
        foreach(var (id,fmg) in textTables){fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));messages.Files.Single(f=>f.ID==id).Bytes=fmg.Write();}
        Directory.CreateDirectory(output);var regPath=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(regPath,target);
        if(new FileInfo(regPath).Length>4799*1024)throw new Exception("Regulation size limit exceeded");
        var restored=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(regPath);
        foreach(var f in target.Files)if(!f.Bytes.SequenceEqual(restored.Files.Single(r=>r.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip failed");
        foreach(var f in target.Files.Where(f=>!targets.ContainsKey(FileName(f))))if(!f.Bytes.SequenceEqual(before[f.Name]))throw new Exception("Unrelated table changed");
        foreach(var (table,param) in targets) {
            var old=PARAM.Read(before[target.Files.Single(f=>FileName(f)==table).Name]);old.ApplyParamdef(param.AppliedParamdef);var current=param.Rows.ToDictionary(r=>r.ID);
            foreach(var r in old.Rows.Where(r=>r.Name?.StartsWith("Tower talisman ")!=true))foreach(var c in r.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,current[r.ID][c.Def.InternalName].Value))throw new Exception("Existing row changed");
        }
        foreach(var (table,id) in required)foreach(var (field,refTable) in References(table,sources[table][id]))if(I(sources[table][id],field)>0&&refTable!="SpEffectVfxParam"&&!required.Contains((refTable,I(sources[table][id],field))))throw new Exception("Unresolved dependency");
        var msgPath=Path.Combine(output,"msg/zhocn/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(msgPath)!);messages.Write(msgPath);
        var msgCheck=BND4.Read(msgPath);foreach(var f in messages.Files)if(!f.Bytes.SequenceEqual(msgCheck.Files.Single(x=>x.ID==f.ID).Bytes))throw new Exception("Text roundtrip failed");
        File.WriteAllLines(Path.Combine(output,"talisman-catalog.txt"),catalog);File.WriteAllLines(Path.Combine(output,"talisman-details.tsv"),details);File.WriteAllLines(Path.Combine(output,"talisman-rarities.tsv"),rarities);
        File.WriteAllText(Path.Combine(output,"talisman-manifest.json"),JsonSerializer.Serialize(manifest,new JsonSerializerOptions{WriteIndented=true}));
        File.WriteAllText(Path.Combine(output,"talisman-build.json"),JsonSerializer.Serialize(new{variants=definitions.Length,prototypes=kinds.Count,dependencies=required.GroupBy(k=>k.Table).ToDictionary(g=>g.Key,g=>g.Count()),sourceSha256=Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(sourcePath))),excludedEffects=new[]{7005660,7005670,7005680,7005690},reason="Critical-hit entry and damage duration are unresolved",inGameVerified=false},new JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified {definitions.Length} talismans across {kinds.Count} prototypes with {required.Count} remapped dependencies; existing rows and spell tables unchanged");
    }
    public static void Verify(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var tables=new[]{"SpEffectParam","BehaviorParam_PC","Bullet","AtkParam_Pc","SpEffectVfxParam","EquipParamAccessory"}.ToDictionary(t=>t,t=>Read(reg,t).Rows.ToDictionary(r=>r.ID));
        var text=ItemTextTables.Read(BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx")));
        var details=File.ReadAllLines(Path.Combine(input,"talisman-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var defs=File.ReadAllLines(Path.Combine(input,"talisman-definitions.tsv")).Select(l=>l.Split('\t')).ToArray();
        foreach(var p in defs) {
            int id=int.Parse(p[0]),source=int.Parse(p[1]),tier=int.Parse(p[3]);var row=tables["EquipParamAccessory"][id];
            if(I(row,"refId")!=Mapped("SpEffectParam",source)||I(row,"rarity")!=tier)throw new Exception("Talisman root/rarity mismatch");
            var effect=tables["SpEffectParam"][I(row,"refId")];
            if(I(effect,"stateInfo") is 303 or 304 or 305 or 306) {
                if(I(row,"accessoryGroup")!=24900+tier||I(effect,"stateInfo")!=303+tier)throw new Exception("On-hit rarity channel/group mismatch");
            }
            if(!text["AccessoryName"].Entries.Any(e=>e.ID==id&&EquipmentStyleAssets.Plain(e.Text)==p[5]))throw new Exception("Talisman name mismatch");
            if(!text["AccessoryInfo"].Entries.Any(e=>e.ID==id&&EquipmentStyleAssets.Plain(e.Text)==p[6]))throw new Exception("Talisman effect description mismatch");
            var d=details[0x20000000u|(uint)id];if(d.Length!=7||new FileInfo(Path.Combine(input,"reward-icons",d[1]+".rgba")).Length!=102400)throw new Exception("Talisman presentation incomplete");
            if(int.Parse(d[1])!=I(row,"iconId"))throw new Exception($"Talisman icon differs between regulation and reward card: {id}");
        }
        int links=0;
        foreach(var (table,rows) in tables)foreach(var row in rows.Values.Where(r=>r.Name?.StartsWith("Tower talisman ")==true)) {
            foreach(var (field,refTable) in References(table,row))if(I(row,field)>0&&refTable!="SpEffectVfxParam") {if(!tables[refTable].ContainsKey(I(row,field)))throw new Exception($"Dangling talisman dependency {table}:{row.ID}/{field}");links++;}
        }
        foreach(var (root,rate) in new[]{(7004840,4f),(7004850,6.5f),(7004860,10f),(7004870,14f)}) {
            var effect=tables["SpEffectParam"][Mapped("SpEffectParam",root)];var proc=tables["SpEffectParam"][I(effect,"accumuOverFireId")];
            if(I(effect,"accumuOverVal")!=6||Convert.ToSingle(proc["changeMpRate"].Value)!=-rate||I(proc,"accumuVal")!=-999)throw new Exception("Spirit extraction mechanics changed");
        }
        Console.WriteLine($"Verified {defs.Length} talisman items, native text/icons, rarity grouping and {links} gameplay dependency references");
    }
}
