using SoulsFormats;
using System.Text.Json;

static class BossAssets {
    public record Part(int Act,int Encounter,int Index,int Model,int Source,int Think,int Runes,string Name,string Label);
    public static Part[] Parts(string root)=>File.ReadAllLines(Path.Combine(root,"boss-pool.txt")).Select(line=>{
        var p=line.Split('|');return new Part(int.Parse(p[0]),int.Parse(p[1]),int.Parse(p[2]),int.Parse(p[3]),int.Parse(p[4]),int.Parse(p[5]),int.Parse(p[6]),p[7],p[8]);
    }).ToArray();
    public static int Npc(Part p,bool elite)=>950000000+p.Index*2+(elite?1:0);
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
    public static void Build(string output,Func<string,PARAM> read,Action<string,PARAM> save,BND4 messages) {
        if(Path.GetFullPath("outputs/erdtree-tower/scene-assets/boss-pool.txt")!=Path.GetFullPath(Path.Combine(output,"boss-pool.txt")))File.Copy("outputs/erdtree-tower/scene-assets/boss-pool.txt",Path.Combine(output,"boss-pool.txt"),true);
        var npcs=read("NpcParam");var ai=read("NpcThinkParam");
        var file=messages.Files.First(f=>f.Name.Contains("NpcName"));var names=FMG.Read(file.Bytes);
        var parts=Parts(output);
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
            names.Entries.Add(new FMG.Entry(text,p.Name));
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
    public static void Verify(string output,Func<string,PARAM> read) {
        var parts=Parts(output);var npcs=read("NpcParam");var ai=read("NpcThinkParam");
        if(parts.Select(p=>p.Encounter).Distinct().Count()!=105||parts.Length!=140||parts.Select(p=>p.Index).Distinct().Count()!=parts.Length)throw new Exception("Boss table coverage mismatch");
        var names=BND4.Read(Path.Combine(output,"msg/zhocn/item_dlc02.msgbnd.dcx")).Files.Where(f=>f.Name.Contains("NpcName")).SelectMany(f=>FMG.Read(f.Bytes).Entries).ToList();
        foreach(var p in parts) {
            if(!ai.Rows.Any(r=>r.ID==p.Think)||!names.Any(e=>e.ID==9600000+p.Index&&e.Text==p.Name))throw new Exception("Boss AI/name missing");
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
            foreach(var p in parts)for(short slot=0;slot<3;slot++) {
                var ev=script.Events.Single(e=>e.ID==basis+3000+p.Index*3+slot);
                var gates=ev.Instructions.Where(i=>i.Bank==3&&i.ID==0).Select(i=>BitConverter.ToUInt32(i.ArgData,4)).ToArray();
                uint member=47000000u+(uint)p.Index*4+(uint)slot;
                if(!gates.SequenceEqual(new[]{47000003u,member,47000003u,member}))throw new Exception("Boss bar flags must be isolated from portal interaction");
                var bars=ev.Instructions.Where(i=>i.Bank==2003&&i.ID==11).ToArray();
                if(bars.Length!=2||bars[0].ArgData[0]!=1||bars[1].ArgData[0]!=0||bars.Any(b=>b.ArgData.Length!=16||BitConverter.ToUInt32(b.ArgData,4)!=basis+850+slot||BitConverter.ToInt16(b.ArgData,8)!=slot||BitConverter.ToInt32(b.ArgData,12)!=9600000+p.Index))throw new Exception("Native boss bar ABI mismatch");
                if(!script.Events.Single(e=>e.ID==0).Instructions.Any(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==ev.ID))throw new Exception("Boss bar event not registered");
            }
        }
        Console.WriteLine("Verified 105 boss encounters, 140 source components, elite variants and native health bar events in all three arenas");
    }
}
