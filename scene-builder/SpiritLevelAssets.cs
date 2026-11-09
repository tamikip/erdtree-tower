using SoulsFormats;

static class SpiritLevelAssets {
    public const int Max=39;
    static int I(PARAM.Row r,string field)=>Convert.ToInt32(r[field].Value);
    static void Set(PARAM.Row r,string field,int value)=>r[field].Value=Convert.ChangeType(value,r[field].Value.GetType());
    public static void PatchTuningBridge(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var original=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="EquipParamGoods");
        var p=PARAM.Read(file.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs","EquipParamGoods.xml")));
        var rows=p.Rows.ToDictionary(r=>r.ID);var before=p.Rows.ToDictionary(r=>r.ID,r=>new PARAM.Row(r));
        var bases=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(r=>r[0]=="spirit").Select(r=>int.Parse(r[1])).Distinct().ToArray();
        var bridges=bases.Select(id=>id+10).ToHashSet();int changed=0;
        foreach(int id in bases) {
            for(int n=0;n<=Max;n++)if(!rows.ContainsKey(id+n))throw new Exception($"Missing spirit level {id+n}");
            var row=rows[id+10];int old=I(row,"reinforceGoodsId");
            if(old!=-1&&old!=id+11)throw new Exception($"Unexpected spirit bridge {id+10}: {old}");
            if(old!=id+11)changed++;
            Set(row,"reinforceGoodsId",id+11);Set(row,"reinforceMaterialId",700000);Set(row,"reinforcePrice",0);
            for(int n=0;n<Max;n++)if(I(rows[id+n],"reinforceGoodsId")!=id+n+1)throw new Exception($"Broken spirit chain at {id+n}");
            if(I(rows[id+Max],"reinforceGoodsId")!=-1)throw new Exception("Spirit +39 must remain capped");
        }
        foreach(var row in p.Rows)foreach(var cell in row.Cells) {
            if(bridges.Contains(row.ID)&&cell.Def.InternalName is "reinforceGoodsId" or "reinforceMaterialId" or "reinforcePrice")continue;
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,before[row.ID][cell.Def.InternalName].Value))throw new Exception("Unrelated spirit field changed");
        }
        file.Bytes=p.Write();Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files)if(!f.Bytes.SequenceEqual(f.Name==file.Name?file.Bytes:original[f.Name]))throw new Exception("Spirit bridge roundtrip mismatch");
        Console.WriteLine($"Verified complete +0..+39 tuning chains for {bases.Length} families; repaired {changed} +10 links; all unrelated fields and tables preserved");
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var originals=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name) {
            var p=PARAM.Read(File_(name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,(name=="SpEffectParam"?"SpEffect":name)+".xml")));return p;
        }
        var goods=Read("EquipParamGoods");var effects=Read("SpEffectParam");var buddies=Read("BuddyParam");
        var g=goods.Rows.ToDictionary(r=>r.ID);var e=effects.Rows.ToDictionary(r=>r.ID);
        var buddyIds=buddies.Rows.Select(r=>r.ID).ToHashSet();
        var bases=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="spirit").Select(p=>int.Parse(p[1])).Distinct().Order().ToArray();
        var scaled=new Dictionary<(int,int,int),int>();var generated=new List<(int id,int trigger,int count)>();
        void Add(PARAM p,Dictionary<int,PARAM.Row> index,PARAM.Row row) {
            if(!index.TryAdd(row.ID,row))throw new Exception($"Spirit level ID collision: {row.ID}");
            p.Rows.Add(row);
        }
        for(int family=0;family<bases.Length;family++) {
            int id=bases[family],fullTrigger=I(g[id+10],"refId_default"),trigger=fullTrigger-10;
            Set(g[id+10],"reinforceGoodsId",id+11);Set(g[id+10],"reinforceMaterialId",700000);Set(g[id+10],"reinforcePrice",0);
            var sources=buddies.Rows.Where(r=>I(r,"triggerSpEffectId")==trigger).ToArray();
            if(sources.Length==0||sources.Length>=1000)throw new Exception($"Spirit buddy source missing: {id}");
            for(int level=11;level<=Max;level++) {
                int nextTrigger=600000000+family*40000+(level-11)*1000;
                Add(effects,e,new PARAM.Row(e[fullTrigger]){ID=nextTrigger,Name=$"Tower spirit trigger {id}+{level}"});
                var item=new PARAM.Row(g[id+10]){ID=id+level,Name=$"Tower spirit {id}+{level}"};
                Set(item,"refId_default",nextTrigger);Set(item,"reinforceGoodsId",level==Max?-1:id+level+1);
                Set(item,"reinforceMaterialId",700000);Set(item,"reinforcePrice",0);Add(goods,g,item);
                for(int slot=0;slot<sources.Length;slot++) {
                    var source=sources[slot];int previous=I(source,"dopingSpEffect_lv9"),last=I(source,"dopingSpEffect_lv10");
                    if(previous<0)previous=buddies.Rows.Where(r=>I(r,"triggerSpEffectId")==I(g[id+9],"refId_default")-9&&I(r,"npcParamId")==I(source,"npcParamId"))
                        .Select(r=>I(r,"dopingSpEffect_lv9")).FirstOrDefault(n=>n>=0,-1);
                    var key=(previous,last,level);
                    if(!scaled.TryGetValue(key,out int effectId)) {
                        effectId=510000000+scaled.Count;
                        if(!e.TryGetValue(previous,out var a)||!e.TryGetValue(last,out var b))throw new Exception($"Missing spirit scaling {id}: {previous}/{last}");
                        var effect=new PARAM.Row(b){ID=effectId,Name=$"Tower spirit scaling {last}+{level}"};
                        foreach(var cell in effect.Cells) {
                            var field=cell.Def.InternalName;var before=a[field].Value;
                            if(cell.Value is float value&&before is float prior&&value!=prior) {
                                float extended=value+(value-prior)*(level-10);
                                if(!float.IsFinite(extended)||extended<0)throw new Exception($"Unsafe spirit scaling: {field}");
                                cell.Value=extended;
                            } else if(field.StartsWith("add")&&field.EndsWith("Status")&&Convert.ToInt32(cell.Value)!=Convert.ToInt32(before))
                                Set(effect,field,Convert.ToInt32(cell.Value)+(Convert.ToInt32(cell.Value)-Convert.ToInt32(before))*(level-10));
                        }
                        Add(effects,e,effect);scaled.Add(key,effectId);
                    }
                    int buddyId=1500000000+family*40000+(level-11)*1000+slot;
                    if(!buddyIds.Add(buddyId))throw new Exception($"Spirit buddy ID collision: {buddyId}");
                    var buddy=new PARAM.Row(source){ID=buddyId,Name=$"Tower spirit buddy {id}+{level}/{slot}"};
                    Set(buddy,"triggerSpEffectId",nextTrigger);
                    // A distinct level-zero trigger avoids indexing beyond the engine's eleven scaling slots.
                    for(int n=0;n<=10;n++)Set(buddy,$"dopingSpEffect_lv{n}",effectId);
                    buddies.Rows.Add(buddy);
                }
                generated.Add((id+level,nextTrigger,sources.Length));
            }
        }
        var tables=new[]{("EquipParamGoods",goods),("SpEffectParam",effects),("BuddyParam",buddies)};
        foreach(var (name,p) in tables){p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_(name).Bytes=p.Write();}
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var file in check.Files) {
            var name=Path.GetFileNameWithoutExtension(file.Name.Replace('\\','/'));
            if(!tables.Any(t=>t.Item1==name)){if(!file.Bytes.SequenceEqual(originals[file.Name]))throw new Exception("Unrelated table changed");continue;}
            var p=tables.Single(t=>t.Item1==name).Item2;
            var old=PARAM.Read(originals[file.Name]);old.ApplyParamdef(p.AppliedParamdef);
            var actual=PARAM.Read(file.Bytes);actual.ApplyParamdef(p.AppliedParamdef);var rows=actual.Rows.ToDictionary(r=>r.ID);
            if(actual.Rows.Count!=p.Rows.Count)throw new Exception("Spirit row count mismatch");
            foreach(var row in old.Rows)foreach(var cell in row.Cells)
                if(name=="EquipParamGoods"&&bases.Contains(row.ID-10)&&cell.Def.InternalName is "reinforceGoodsId" or "reinforceMaterialId" or "reinforcePrice")continue;
                else
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing spirit data changed: {name}/{row.ID}");
            foreach(var row in p.Rows)foreach(var cell in row.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception("Spirit output mismatch");
        }
        foreach(var row in generated) {
            if(I(g[row.id],"refId_default")!=row.trigger||!e.ContainsKey(row.trigger)||buddies.Rows.Count(b=>I(b,"triggerSpEffectId")==row.trigger)!=row.count)throw new Exception("Spirit summon reference mismatch");
        }
        const string msg="msg/zhocn/item_dlc02.msgbnd.dcx";
        var itemText=BND4.Read(Path.Combine(input,msg));int names=0;
        foreach(var file in itemText.Files) {
            var name=Path.GetFileNameWithoutExtension(file.Name);
            if(!name.StartsWith("GoodsName")&&!name.StartsWith("GoodsInfo")&&!name.StartsWith("GoodsCaption"))continue;
            var fmg=FMG.Read(file.Bytes);
            foreach(var id in bases) {
                var source=fmg.Entries.SingleOrDefault(x=>x.ID==id+10);
                if(string.IsNullOrWhiteSpace(source?.Text))continue;
                var baseName=fmg.Entries.SingleOrDefault(x=>x.ID==id)?.Text;
                for(int level=11;level<=Max;level++) {
                    if(fmg.Entries.Any(x=>x.ID==id+level))throw new Exception("Spirit text ID collision");
                    var text=name.StartsWith("GoodsName")?(baseName??throw new Exception("Missing base spirit name"))+$"＋{level}":source!.Text;
                    fmg.Entries.Add(new FMG.Entry(id+level,text));if(name.StartsWith("GoodsName"))names++;
                }
            }
            file.Bytes=fmg.Write();
        }
        if(names!=generated.Count)throw new Exception($"Spirit names missing: {names}/{generated.Count}");
        var msgOut=Path.Combine(output,msg);Directory.CreateDirectory(Path.GetDirectoryName(msgOut)!);itemText.Write(msgOut);
        var textCheck=BND4.Read(msgOut);
        foreach(var file in itemText.Files)if(!file.Bytes.SequenceEqual(textCheck.Files.Single(f=>f.ID==file.ID).Bytes))throw new Exception("Spirit text roundtrip failed");
        const string menuPath="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,menuPath));var menuFile=menu.Files.Single(f=>f.ID==33);var menuText=FMG.Read(menuFile.Bytes);
        var tune=menuText.Entries.Single(x=>x.ID==9500156);tune.Text=tune.Text.Replace("+10","+39");
        menuFile.Bytes=menuText.Write();var menuOut=Path.Combine(output,menuPath);menu.Write(menuOut);
        foreach(var file in BND4.Read(menuOut).Files)if(!file.Bytes.SequenceEqual(menu.Files.Single(f=>f.ID==file.ID).Bytes))throw new Exception("Spirit menu roundtrip failed");
        Console.WriteLine($"Verified {bases.Length} spirit families through +{Max}: {generated.Count} new goods/triggers, {scaled.Count} scaling effects; original rows preserved.");
    }
    public static void Inspect(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        PARAM Read(string name) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")!,(name=="SpEffectParam"?"SpEffect":name)+".xml")));return p;
        }
        var goods=Read("EquipParamGoods");var effects=Read("SpEffectParam");var buddies=Read("BuddyParam");
        var npcs=Read("NpcParam");
        foreach(var id in new[]{121000000,32511030,42600110,25000010,31000020,39100000})
            if(npcs.Rows.SingleOrDefault(r=>r.ID==id) is {} npc)
                Console.WriteLine($"ALLY NPC {id}: "+string.Join(" ",npc.Cells.Where(c=>c.Def.InternalName.Contains("team",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.Contains("spEffect")||c.Def.InternalName is "hp" or "getSoul" or "npcType" or "itemLotId_enemy" or "itemLotId_map").Select(c=>$"{c.Def.InternalName}={c.Value}")));
        foreach(var id in new[]{200009,200010,201009,201010,2200009,2200010,233009,233010}) {
            var row=goods.Rows.SingleOrDefault(r=>r.ID==id);if(row==null)continue;
            Console.WriteLine($"GOODS {id}: "+string.Join(" ",row.Cells.Where(c=>c.Def.InternalName.Contains("refId")||c.Def.InternalName.Contains("reinforce")||c.Def.InternalName.Contains("Level")||c.Def.InternalName.Contains("icon")).Select(c=>$"{c.Def.InternalName}={c.Value}")));
            var refid=Convert.ToInt32(row["refId_default"].Value);
            var effect=effects.Rows.Single(r=>r.ID==refid);
            var zero=effects.Rows.SingleOrDefault(r=>r.ID==refid-refid%1000);
            if(zero!=null)Console.WriteLine("TRIGGER DIFF "+string.Join(" ",effect.Cells.Where(c=>!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,zero[c.Def.InternalName].Value)).Select(c=>$"{c.Def.InternalName}:{zero[c.Def.InternalName].Value}->{c.Value}")));
            Console.WriteLine("SUMMON "+string.Join(" ",effect.Cells.Where(c=>c.Def.InternalName.Contains("buddy",StringComparison.OrdinalIgnoreCase)||c.Def.InternalName.Contains("reinforce",StringComparison.OrdinalIgnoreCase)).Select(c=>$"{c.Def.InternalName}={c.Value}")));
            Console.WriteLine($"EFFECT {refid}: "+string.Join(" ",effect.Cells.Where(c=>c.Value is int n&&n!=0&&n!=-1 || c.Value is float f&&f!=0&&f!=1).Select(c=>$"{c.Def.InternalName}={c.Value}")));
            foreach(var buddy in buddies.Rows.Where(r=>Convert.ToInt32(r["triggerSpEffectId"].Value)>=refid-10&&Convert.ToInt32(r["triggerSpEffectId"].Value)<=refid))
                Console.WriteLine($"BUDDY {buddy.ID}: "+string.Join(" ",buddy.Cells.Where(c=>c.Value is int n&&n!=0&&n!=-1).Select(c=>$"{c.Def.InternalName}={c.Value}")));
        }
        foreach(var buddy in buddies.Rows.Take(3))
            Console.WriteLine($"BUDDY SAMPLE {buddy.ID}: "+string.Join(" ",buddy.Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
        foreach(var pair in buddies.Rows.Where(r=>Convert.ToInt32(r["triggerSpEffectId"].Value)>0).Select(r=>(a:Convert.ToInt32(r["dopingSpEffect_lv9"].Value),b:Convert.ToInt32(r["dopingSpEffect_lv10"].Value))).Distinct()) {
            var a=effects.Rows.SingleOrDefault(r=>r.ID==pair.a);var b=effects.Rows.SingleOrDefault(r=>r.ID==pair.b);if(a==null||b==null)continue;
            Console.WriteLine($"SCALING {pair}: "+string.Join(" ",b.Cells.Where(c=>!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,a[c.Def.InternalName].Value)).Select(c=>$"{c.Def.InternalName}:{a[c.Def.InternalName].Value}->{c.Value}")));
        }
    }
}
