using SoulsFormats;
using System.Text.Json;

static class CustomSpiritAssets {
    record Spec(int Id,int Model,int Npc,int Think,int Tier,int Hp,int IconSource,string Name,string Info);
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    static void Set(PARAM.Row row,string field,int value)=>row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var specs=File.ReadAllLines(Path.Combine(input,"custom-spirit-definitions.tsv")).Select(l=>{
            var p=l.Split('|');return new Spec(int.Parse(p[0]),int.Parse(p[1]),int.Parse(p[2]),int.Parse(p[3]),int.Parse(p[4]),int.Parse(p[5]),int.Parse(p[6]),p[7],p[8]);
        }).ToArray();
        if(specs.Length!=12||specs.Select(s=>s.Id).Distinct().Count()!=12||specs.Any(s=>s.Id%1000!=0||s.Tier is <2 or >3))throw new Exception("Invalid custom spirit definitions");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var original=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        var tables=new Dictionary<string,PARAM>();
        foreach(var name in new[]{"EquipParamGoods","SpEffectParam","BuddyParam","NpcParam","NpcThinkParam"}) {
            var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,(name=="SpEffectParam"?"SpEffect":name)+".xml")));tables.Add(name,p);
        }
        var index=tables.ToDictionary(t=>t.Key,t=>t.Value.Rows.ToDictionary(r=>r.ID));
        PARAM.Row Row(string table,int id)=>index[table].TryGetValue(id,out var row)?row:throw new Exception($"Missing source {table}/{id}");
        void Add(string table,PARAM.Row row) {
            if(!index[table].TryAdd(row.ID,row))throw new Exception($"Custom spirit ID collision: {table}/{row.ID}");
            tables[table].Rows.Add(row);
        }
        var ally=Row("NpcParam",121000000);var allyThink=Row("NpcThinkParam",121000000);
        var sourceBuddy=Row("BuddyParam",20000000);
        var mapNames=File.ReadAllLines(Path.Combine(input,"arenas-built.txt")).Where(l=>!l.StartsWith('#')&&!string.IsNullOrWhiteSpace(l)).Select(l=>l.Split('|')[1])
            .Concat(File.ReadAllLines(Path.Combine(input,"battlefields.txt")).Select(l=>l.Split('|')[1])).Distinct();
        var arenaModels=mapNames.ToDictionary(m=>m,m=>MSBE.Read(Path.Combine(input,"map/mapstudio",m+".msb.dcx")).Models.Enemies.Select(e=>e.Name).ToHashSet());
        var audit=new List<object>();
        for(int family=0;family<specs.Length;family++) {
            var s=specs[family];int npcId=100000000+s.Model*10000+8000+family,thinkId=npcId;
            int sourceMaps=arenaModels.Values.Count(models=>models.Contains($"c{s.Model:D4}"));
            if(sourceMaps==0)throw new Exception($"Summon model c{s.Model:D4} not present in existing arena resources");
            var npc=new PARAM.Row(Row("NpcParam",s.Npc)){ID=npcId,Name="Tower custom spirit "+s.Name};
            Set(npc,"teamType",I(ally,"teamType"));Set(npc,"hp",s.Hp);Set(npc,"getSoul",0);
            Set(npc,"itemLotId_enemy",-1);Set(npc,"itemLotId_map",-1);
            // Use the native spirit's allegiance/scaling effects; retain model-specific move effects.
            foreach(int slot in Enumerable.Range(0,15).Concat(Enumerable.Range(25,7)))Set(npc,$"spEffectID{slot}",I(ally,$"spEffectID{slot}"));
            for(int slot=15;slot<25;slot++)if(I(npc,$"spEffectID{slot}")==4301)Set(npc,$"spEffectID{slot}",4302);
            Add("NpcParam",npc);
            var think=new PARAM.Row(Row("NpcThinkParam",s.Think)){ID=thinkId,Name="Tower custom spirit AI "+s.Name};
            Set(think,"isBuddyAI",1);
            foreach(var field in new[]{"maxBackhomeDist","backhomeDist","backhomeBattleDist"})Set(think,field,I(allyThink,field));
            Add("NpcThinkParam",think);
            for(int level=0;level<=SpiritLevelAssets.Max;level++) {
                int trigger=610000000+family*40000+level*1000,buddyId=1600000000+family*40000+level*1000;
                var triggerEffect=new PARAM.Row(Row("SpEffectParam",200000)){ID=trigger,Name=$"Tower custom spirit trigger {s.Id}+{level}"};
                Add("SpEffectParam",triggerEffect);
                int scaling=level<=10?290000+level:tables["SpEffectParam"].Rows.Single(r=>r.Name==$"Tower spirit scaling 290010+{level}").ID;
                var buddy=new PARAM.Row(sourceBuddy){ID=buddyId,Name=$"Tower custom spirit buddy {s.Id}+{level}"};
                Set(buddy,"triggerSpEffectId",trigger);Set(buddy,"npcParamId",npcId);Set(buddy,"npcThinkParamId",thinkId);
                Set(buddy,"npcParamId_ridden",-1);Set(buddy,"npcThinkParamId_ridden",-1);Set(buddy,"npcPlayerInitParamId",-1);
                // Enemy animations do not necessarily contain the native ashes' summon animation.
                Set(buddy,"generateAnimId",-1);
                for(int n=0;n<=10;n++)Set(buddy,$"dopingSpEffect_lv{n}",scaling);
                Add("BuddyParam",buddy);
                var goods=new PARAM.Row(Row("EquipParamGoods",200000+Math.Min(level,10))){ID=s.Id+level,Name=$"Tower custom spirit {s.Name}+{level}"};
                Set(goods,"refId_default",trigger);Set(goods,"iconId",I(Row("EquipParamGoods",s.IconSource),"iconId"));
                Set(goods,"rarity",s.Tier);Set(goods,"reinforceGoodsId",level==SpiritLevelAssets.Max?-1:s.Id+level+1);
                Set(goods,"reinforceMaterialId",700000);Set(goods,"reinforcePrice",0);Set(goods,"consumeMP",0);Set(goods,"consumeHP",-1);
                Add("EquipParamGoods",goods);
                if(I(npc,"teamType")!=47||I(think,"isBuddyAI")!=1||I(npc,"getSoul")!=0||I(npc,"itemLotId_enemy")!=-1||
                    !index["SpEffectParam"].ContainsKey(scaling)||I(buddy,"triggerSpEffectId")!=I(goods,"refId_default"))
                    throw new Exception("Custom spirit allegiance/reference mismatch");
            }
            audit.Add(new {s.Id,s.Model,s.Name,s.Tier,s.Hp,npcId,thinkId,levels=40,team=47,sourceMaps,liveTested=false});
        }
        foreach(var (name,p) in tables){p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_(name).Bytes=p.Write();}
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var file in check.Files) {
            var name=Path.GetFileNameWithoutExtension(file.Name.Replace('\\','/'));
            if(!tables.TryGetValue(name,out var p)) {if(!file.Bytes.SequenceEqual(original[file.Name]))throw new Exception("Unrelated table changed");continue;}
            var before=PARAM.Read(original[file.Name]);before.ApplyParamdef(p.AppliedParamdef);
            var actual=PARAM.Read(file.Bytes);actual.ApplyParamdef(p.AppliedParamdef);var rows=actual.Rows.ToDictionary(r=>r.ID);
            if(actual.Rows.Count!=p.Rows.Count)throw new Exception("Custom spirit row count mismatch");
            foreach(var row in before.Rows)foreach(var cell in row.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing row changed: {name}/{row.ID}");
            foreach(var row in p.Rows)foreach(var cell in row.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception("Custom spirit serialization mismatch");
        }
        const string message="msg/zhocn/item_dlc02.msgbnd.dcx";
        var text=BND4.Read(Path.Combine(input,message));
        foreach(var table in new[]{"GoodsName","GoodsInfo","GoodsCaption"}) {
            var file=text.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name).Split("_dlc")[0]==table&&FMG.Read(f.Bytes).Entries.Any(e=>e.ID==200000&&!string.IsNullOrWhiteSpace(e.Text)));
            var fmg=FMG.Read(file.Bytes);
            foreach(var s in specs)for(int level=0;level<=SpiritLevelAssets.Max;level++) {
                if(fmg.Entries.Any(e=>e.ID==s.Id+level))throw new Exception("Custom spirit text collision");
                var value=table=="GoodsName"?s.Name+(level==0?"":$"＋{level}"):s.Info+(table=="GoodsCaption"?"\n\n黄金树爬塔的召唤骨灰。可强化至 +39。\n每次召唤消耗一份骨灰；召唤师在胜利时按存活情况返还。":"");
                fmg.Entries.Add(new FMG.Entry(s.Id+level,value));
            }
            file.Bytes=fmg.Write();
        }
        var msgOut=Path.Combine(output,message);Directory.CreateDirectory(Path.GetDirectoryName(msgOut)!);text.Write(msgOut);
        foreach(var file in BND4.Read(msgOut).Files)if(!file.Bytes.SequenceEqual(text.Files.Single(f=>f.ID==file.ID).Bytes))throw new Exception("Custom spirit text roundtrip failed");
        var catalog=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).ToList();
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).ToList();
        var rarities=File.ReadAllLines(Path.Combine(input,"reward-rarities.tsv")).ToList();
        foreach(var s in specs) {
            uint full=0x40000000u|(uint)s.Id;int icon=I(Row("EquipParamGoods",s.Id),"iconId");
            if(catalog.Any(l=>l.StartsWith($"spirit|{s.Id}|"))||details.Any(l=>l.StartsWith(full+"\t"))||rarities.Any(l=>l.StartsWith(full+"\t")))throw new Exception("Custom spirit catalog collision");
            if(new FileInfo(Path.Combine(input,"reward-icons",icon+".rgba")).Length!=160*160*4)throw new Exception("Custom spirit icon missing");
            catalog.Add($"spirit|{s.Id}|{s.Name}");rarities.Add($"{full}\t{s.Tier}");
            details.Add($"{full}\t{icon}\t{s.Name}\t{s.Info}\t{s.Info}\\n\\n可强化至 +39。每次召唤消耗一份骨灰。\t召唤消耗：0 专注值、0 生命值。\\n装备到快捷栏后，在战斗区域召唤。\t");
        }
        File.WriteAllLines(Path.Combine(output,"reward-catalog.txt"),catalog);
        File.WriteAllLines(Path.Combine(output,"reward-details.tsv"),details);
        File.WriteAllLines(Path.Combine(output,"reward-rarities.tsv"),rarities);
        File.WriteAllText(Path.Combine(output,"custom-spirit-build.json"),JsonSerializer.Serialize(audit,new JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified {specs.Length} custom spirits × 40 levels, friendly NPC/AI, no drops, summon references and texts; model sources found across {arenaModels.Count} maps, existing rows preserved. Live behavior not yet tested.");
    }
}
