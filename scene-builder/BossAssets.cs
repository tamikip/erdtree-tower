using SoulsFormats;
using System.Text.Json;

static class BossAssets {
    public record Part(int Act,int Encounter,int Index,int Model,int Source,int Think,int Runes,string Name,string Label);
    public static Part[] Parts(string root)=>File.ReadAllLines(Path.Combine(root,"boss-pool.txt")).Select(line=>{
        var p=line.Split('|');return new Part(int.Parse(p[0]),int.Parse(p[1]),int.Parse(p[2]),int.Parse(p[3]),int.Parse(p[4]),int.Parse(p[5]),int.Parse(p[6]),p[7],p[8]);
    }).ToArray();
    public static int Npc(Part p,bool elite)=>950000000+p.Index*2+(elite?1:0);
    public static Dictionary<int,string> BarNames(string root) {
        var parts=Parts(root).ToDictionary(p=>p.Index);
        var names=parts.Values.ToDictionary(p=>9600000+p.Index,p=>p.Name);
        var assigned=new HashSet<int>();
        foreach(var line in File.ReadAllLines(Path.Combine(root,"boss-bar-names.txt"))) {
            var p=line.Split('|');int index=int.Parse(p[0]);
            if(p.Length!=4||!assigned.Add(index)||!parts.TryGetValue(index,out var part)||part.Model!=int.Parse(p[1])||part.Source!=int.Parse(p[2])||string.IsNullOrWhiteSpace(p[3]))
                throw new Exception($"Invalid individual boss name: {line}");
            names[9600000+index]=p[3];
        }
        foreach(var group in parts.Values.GroupBy(p=>p.Encounter).Where(g=>g.Count()>1))
            if(group.Any(p=>!assigned.Contains(p.Index)))throw new Exception($"Missing individual names for encounter {group.Key}");
        return names;
    }
    public static void TestBarNames(string root) {
        var parts=Parts(root);var names=BarNames(root);
        foreach(var (index,name) in new[]{(61,"熔炉骑士"),(62,"混种战士"),(68,"诺克斯修士"),(69,"诺克斯剑士"),(83,"“病花”米兰达"),(84,"恶兆猎人"),(20,"小恶魔"),(45,"结晶人（手杖）"),(46,"结晶人（矛）")})
            if(names[9600000+index]!=name)throw new Exception($"Individual boss bar mismatch: {index}: '{names[9600000+index]}' != '{name}'");
        foreach(var group in parts.GroupBy(p=>p.Encounter))foreach(var p in group) {
            var name=names[9600000+p.Index];
            if(group.Count()==1&&name!=p.Name)throw new Exception("Single boss name changed");
            if(group.Count()>1&&(name.Contains('＋')||name.Contains('+')||name.Contains("双人组")||name.Contains("三人组")||name.Contains("双胞胎")))throw new Exception($"Group label on individual bar: {p.Index}");
        }
        if(names.Count!=parts.Length)throw new Exception("Boss name IDs changed");
        Console.WriteLine($"Verified individual bar names for {parts.GroupBy(p=>p.Encounter).Count(g=>g.Count()>1)} multi-enemy encounters; single boss names and IDs preserved");
    }
    public static void Export(string output) {
        using var document=JsonDocument.Parse(File.ReadAllText("work/boss-candidates.json"));
        var lines=new List<string>();int encounter=0,index=0;
        foreach(var row in document.RootElement.EnumerateArray()) {
            int act=row.GetProperty("act").GetInt32();string label=row.GetProperty("name").GetString();
            foreach(var member in row.GetProperty("members").EnumerateArray()) {
                string map=member.GetProperty("map").GetString();
                var msb=MSBE.Read(Path.Combine("work/boss-report/maps",map+".msb.dcx"));
                var part=msb.Parts.Enemies.Single(e=>e.Name==member.GetProperty("name").GetString());
                int model=int.Parse(part.ModelName[1..]);
                if(model==0||part.NPCParamID<0||part.ThinkParamID<0)throw new Exception($"Unsupported boss source: {label}/{part.Name}");
                lines.Add($"{act}|{encounter}|{index++}|{model}|{part.NPCParamID}|{part.ThinkParamID}|{(act==1?9000:act==2?18000:30000)}|{label}|{part.Name}|{(member.GetProperty("major").GetBoolean()?1:0)}");
            }
            encounter++;
        }
        Directory.CreateDirectory(output);File.WriteAllLines(Path.Combine(output,"boss-pool.txt"),lines);
        Console.WriteLine($"Exported {encounter} encounters / {index} native components");
    }
    public static void Build(string source,string output,Func<string,PARAM> read,Action<string,PARAM> save,BND4 messages) {
        foreach(var name in new[]{"boss-pool.txt","boss-bar-names.txt"}) {
            var input=Path.GetFullPath(Path.Combine(source,name));var target=Path.GetFullPath(Path.Combine(output,name));
            if(input!=target)File.Copy(input,target,true);
        }
        var npcs=read("NpcParam");var ai=read("NpcThinkParam");
        var file=messages.Files.First(f=>f.Name.Contains("NpcName"));var names=FMG.Read(file.Bytes);
        var parts=Parts(output);var barNames=BarNames(output);
        foreach(var p in parts) {
            if(!ai.Rows.Any(r=>r.ID==p.Think))throw new Exception($"Missing boss AI: {p.Name}");
            foreach(bool elite in new[]{false,true}) {
                int id=Npc(p,elite);if(npcs.Rows.Any(r=>r.ID==id))throw new Exception("Boss param collision");
                var row=new PARAM.Row(npcs.Rows.Single(r=>r.ID==p.Source)){ID=id,Name=p.Name};
                if(p.Label.StartsWith("tower_champion_")&&!elite)row["hp"].Value=Convert.ChangeType(Convert.ToInt64(row["hp"].Value)*3,row["hp"].Value.GetType());
                int runes=(elite?p.Runes/3:p.Runes)/parts.Count(member=>member.Encounter==p.Encounter);
                foreach(var (field,value) in new[]{("itemLotId_enemy",-1),("itemLotId_map",-1),("isSoulGetByBoss",0),("disableRespawn",0),("getSoul",runes)})row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
                npcs.Rows.Add(row);
            }
            int text=9600000+p.Index;if(names.Entries.Any(e=>e.ID==text))throw new Exception("Boss name collision");
            names.Entries.Add(new FMG.Entry(text,barNames[text]));
        }
        names.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=names.Write();save("NpcParam",npcs);
    }
    public static void Events(EMEVD script,uint basis,string output) {
        foreach(var p in Parts(output))for(short slot=0;slot<3;slot++) {
            uint flag=47000000u+(uint)p.Index*4+(uint)slot;
            var ev=new EMEVD.Event(basis+3000+p.Index*3+slot,EMEVD.Event.RestBehaviorType.Restart);
            ev.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,47000003u}));
            ev.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,flag}));
            ev.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
            ev.Instructions.Add(new EMEVD.Instruction(2003,11,new object[]{(byte)1,basis+850u+(uint)slot,slot,9600000+p.Index}));
            ev.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)-1,(byte)0,(byte)0,47000003u}));
            ev.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)-1,(byte)0,(byte)0,flag}));
            ev.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)-1}));
            ev.Instructions.Add(new EMEVD.Instruction(2003,11,new object[]{(byte)0,basis+850u+(uint)slot,slot,9600000+p.Index}));
            ev.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
            if(script.Events.Any(e=>e.ID==ev.ID))throw new Exception("Boss health bar event collision");
            script.Events.Add(ev);script.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,(uint)ev.ID,0u}));
        }
    }
    // 之后追加的 Boss（boss-pool.txt 末尾）：为还没有参数行的部件补上 950000000 + 序号×2（精英 +1），
    // 做法与 Build + BalanceAssets 相同（复制原版行、去掉掉落、允许重复遭遇、按幕给卢恩、生命与卢恩按爬塔倍率）。已有行不动，可重复运行；
    // 换了来源行的重建；蕾娜菈二阶段原版战斗实体是 20310024 / Think 20310000。
    // 之后再运行 tower-scaling-patch，把原版区域强度换成爬塔档位。
    static readonly string[] Managed={"hp","itemLotId_enemy","itemLotId_map","isSoulGetByBoss","getSoul","disableRespawn"};
    static bool SourceChanged(PARAM.Row row,PARAM.Row source)=>row.Cells.Any(c=>{
        var name=c.Def.InternalName;var value=source[name].Value;
        if(Managed.Contains(name))return false;
        if(name.StartsWith("spEffectID")&&(TowerScalingAssets.AreaScaling(Convert.ToInt32(c.Value))||TowerScalingAssets.AreaScaling(Convert.ToInt32(value))))return false;
        return !System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,value);
    });
    public static void Append(string input,string output,int? onlyIndex=null) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name) {var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        var npcs=Read("NpcParam");var ai=Read("NpcThinkParam");var before=npcs.Rows.Count;var original=npcs.Rows.ToDictionary(r=>r.ID,r=>new PARAM.Row(r));
        var parts=Parts(input);var added=new List<int>();var rebuilt=new List<int>();
        foreach(var p in parts.Where(p=>!onlyIndex.HasValue||p.Index==onlyIndex.Value)) {
            if(!ai.Rows.Any(r=>r.ID==p.Think))throw new Exception($"Missing boss AI: {p.Name}");
            var source=npcs.Rows.SingleOrDefault(r=>r.ID==p.Source)??throw new Exception($"Missing boss source parameters: {p.Name}");
            if(p.Model==2031) {
                var think=ai.Rows.Single(r=>r.ID==p.Think);
                if(p.Source!=20310024||p.Think!=20310000||Convert.ToInt32(think["logicId"].Value)!=203100||Convert.ToInt32(think["battleGoalID"].Value)!=203100)
                    throw new Exception("Rennala must use the original c2031_9000 combat NPC and battle AI, not generic idle AI");
            }
            foreach(bool elite in new[]{false,true}) {
                int id=Npc(p,elite);var existing=npcs.Rows.SingleOrDefault(r=>r.ID==id);
                if(existing!=null) {
                    if(!SourceChanged(existing,source))continue;
                    if(p.Index<140)throw new Exception($"Only appended bosses may change their source: {p.Name}");
                    npcs.Rows.Remove(existing);rebuilt.Add(id);
                }
                if(p.Label.StartsWith("tower_champion_"))throw new Exception("Champion bosses must come from the full build");
                var row=new PARAM.Row(source){ID=id,Name=p.Name};
                int runes=(elite?p.Runes/3:p.Runes)/parts.Count(member=>member.Encounter==p.Encounter);
                foreach(var (field,value) in new (string,object)[]{("itemLotId_enemy",-1),("itemLotId_map",-1),("isSoulGetByBoss",0),("disableRespawn",0),
                    ("getSoul",BalanceAssets.Scale(runes)),("hp",BalanceAssets.ScaleHp(Convert.ToInt64(source["hp"].Value)))})row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
                npcs.Rows.Add(row);if(existing==null)added.Add(id);
            }
        }
        npcs.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_("NpcParam").Bytes=npcs.Write();
        var others=reg.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))!="NpcParam").ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        if(new FileInfo(path).Length>4799*1024)throw new Exception("Regulation exceeds the game's size limit");
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>others.ContainsKey(f.Name)))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var after=PARAM.Read(check.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam").Bytes);after.ApplyParamdef(npcs.AppliedParamdef);
        if(after.Rows.Count!=before+added.Count)throw new Exception("Unexpected NpcParam row count");
        foreach(var row in after.Rows.Where(r=>original.ContainsKey(r.ID)&&!rebuilt.Contains(r.ID)))foreach(var cell in row.Cells)
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,original[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing NpcParam changed: {row.ID}");
        Console.WriteLine($"Added {added.Count} late boss parameter rows ({string.Join(",",added)}), rebuilt {rebuilt.Count} with a new source ({string.Join(",",rebuilt)}); other rows and params unchanged");
    }
    public static void Verify(string output,Func<string,PARAM> read) {
        var parts=Parts(output);var barNames=BarNames(output);var npcs=read("NpcParam");var ai=read("NpcThinkParam");
        // 原有 105 组 140 个部件；之后追加 13 个单部件 Boss（追忆与 DLC），再追加 5 个 DLC Boss（序号 153–157，仅单人）、碎星拉塔恩（158），
        // 以及只当第一幕精英的黑夜骑兵、混种战士、提比亚的唤声船（159–161，仅单人）
        if(parts.Select(p=>p.Encounter).Distinct().Count()!=127||parts.Length!=162||parts.Select(p=>p.Index).Distinct().Count()!=parts.Length)throw new Exception("Boss table coverage mismatch");
        var names=BND4.Read(Path.Combine(output,"msg/zhocn/item_dlc02.msgbnd.dcx")).Files.Where(f=>f.Name.Contains("NpcName")).SelectMany(f=>FMG.Read(f.Bytes).Entries).ToList();
        foreach(var p in parts) {
            if(!ai.Rows.Any(r=>r.ID==p.Think)||!names.Any(e=>e.ID==9600000+p.Index&&e.Text==barNames[e.ID]))throw new Exception("Boss AI/name missing");
            var source=npcs.Rows.Single(r=>r.ID==p.Source);
            foreach(bool elite in new[]{false,true}) {
                var row=npcs.Rows.Single(r=>r.ID==Npc(p,elite));
                bool champion=p.Label.StartsWith("tower_champion_")&&!elite;
                if(Convert.ToInt64(row["hp"].Value)!=BalanceAssets.ScaleHp(Convert.ToInt64(source["hp"].Value)*(champion?3:1)))throw new Exception("Boss health mismatch");
                if(Convert.ToInt32(row["disableRespawn"].Value)!=0)throw new Exception("Tower boss must allow repeat encounters");
                foreach(var cell in row.Cells.Where(c=>!new[]{"hp","itemLotId_enemy","itemLotId_map","isSoulGetByBoss","getSoul","disableRespawn"}.Contains(c.Def.InternalName)))
                    if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,source[cell.Def.InternalName].Value))throw new Exception("Boss source parameters changed");
                int runes=(elite?p.Runes/3:p.Runes)/parts.Count(member=>member.Encounter==p.Encounter);
                if(Convert.ToInt32(row["getSoul"].Value)!=BalanceAssets.Scale(runes)||Convert.ToInt32(row["itemLotId_enemy"].Value)!=-1||Convert.ToInt32(row["itemLotId_map"].Value)!=-1)throw new Exception("Boss drops mismatch");
            }
        }
        for(uint map=0;map<3;map++) {
            uint basis=45000000+map*10000;
            var script=EMEVD.Read(Path.Combine(output,$"event/m45_0{map}_00_00.emevd.dcx"));
            // 序号 153 起的 Boss 没有地图血条事件：原生 DLL 按句柄绑定血条（boss_bars.rs）
            foreach(var p in parts.Where(p=>p.Index<153))for(short slot=0;slot<3;slot++) {
                var ev=script.Events.Single(e=>e.ID==basis+3000+p.Index*3+slot);
                var gates=ev.Instructions.Where(i=>i.Bank==3&&i.ID==0).Select(i=>BitConverter.ToUInt32(i.ArgData,4)).ToArray();
                uint member=47000000u+(uint)p.Index*4+(uint)slot;
                if(!gates.SequenceEqual(new[]{47000003u,member,47000003u,member}))throw new Exception("Boss bar flags must be isolated from portal interaction");
                var bars=ev.Instructions.Where(i=>i.Bank==2003&&i.ID==11).ToArray();
                if(bars.Length!=2||bars[0].ArgData[0]!=1||bars[1].ArgData[0]!=0||bars.Any(b=>b.ArgData.Length!=16||BitConverter.ToUInt32(b.ArgData,4)!=basis+850+slot||BitConverter.ToInt16(b.ArgData,8)!=slot||BitConverter.ToInt32(b.ArgData,12)!=9600000+p.Index))throw new Exception("Native boss bar ABI mismatch");
                if(!script.Events.Single(e=>e.ID==0).Instructions.Any(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==ev.ID))throw new Exception("Boss bar event not registered");
            }
        }
        Console.WriteLine("Verified 123 boss encounters, 158 source components, elite variants and native health bar events in all three arenas");
    }
}
