using SoulsFormats;

static class FirstActEliteAssets {
    public static void ScaleNew(string input,string output) {
        var path=Path.Combine(output,"regulation.bin");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam");
        var param=PARAM.Read(file.Bytes);param.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml"));
        var tiers=TowerScalingAssets.Tiers(input)[1];
        foreach(var row in param.Rows.Where(r=>Npc(r.ID)))TowerScalingAssets.Apply(row,tiers[row.ID%2==1?1:2]);
        file.Bytes=param.Write();SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
    }
    public static bool Npc(int id)=>id>=950000322&&id<=950000327;
    public static bool Slot(uint id)=>id>=48030294&&id<=48030299;
    public static void Add(string input,MSBE map,EMEVD events) {
        var source=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_02_00_00.msb.dcx"));
        var actors=source.Parts.Enemies.Where(p=>Slot(p.EntityID)&&Npc(p.NPCParamID)).ToArray();
        if(actors.Length!=6)throw new Exception("First-act elite source slots missing");
        var anchor=map.Parts.Enemies.First(p=>p.EntityID>=48010000&&p.EntityID<48010300);
        var init=events.Events.Single(e=>e.ID==0);var added=new List<EMEVD.Instruction>();
        for(uint slot=0;slot<12;slot++) {
            uint id=PartyEnemyEvents.Region(48030294,slot);
            if(map.Regions.GetEntries().Any(r=>r.EntityID==id))continue;
            var region=map.Regions.GetEntries().Single(r=>r.EntityID==PartyEnemyEvents.Region(48010000,slot)).DeepCopy();
            region.EntityID=id;region.Name="tower_act1_extra_position_"+slot;region.ActivationPartName=null;map.Regions.Add(region);
        }
        foreach(var original in actors) {
            uint id=original.EntityID;
            if(!map.Parts.Enemies.Any(p=>p.EntityID==id)) {
                var part=(MSBE.Part.Enemy)original.DeepCopy();part.Name="tower_party_actor_"+id;
                part.Position=anchor.Position;part.Rotation=anchor.Rotation;part.CollisionPartName=anchor.CollisionPartName;part.WalkRouteName=null;part.PlatoonID=-1;
                part.InstanceID=map.Parts.Enemies.Where(p=>p.ModelName==part.ModelName).Select(p=>p.InstanceID).DefaultIfEmpty(-1).Max()+1;
                PartyPlacement.Prepare(part);
                if(!map.Models.Enemies.Any(m=>m.Name==part.ModelName))map.Models.Enemies.Add((MSBE.Model.Enemy)source.Models.Enemies.Single(m=>m.Name==part.ModelName).DeepCopy());
                map.Parts.Enemies.Add(part);added.Add(new(2004,5,new object[]{id,(byte)0}));
                foreach(var flag in new[]{PartyEnemyEvents.Request(id),PartyEnemyEvents.Ready(id)})added.Add(new(2003,66,new object[]{(byte)0,flag,(byte)0}));
                foreach(var ev in new[]{PartyEnemyEvents.Create(id),PartyEnemyEvents.Battle(id)}) {
                    if(events.Events.Any(e=>e.ID==ev.ID))throw new Exception("First-act elite event collision");
                    events.Events.Add(ev);added.Add(new(2000,0,new object[]{0,(uint)ev.ID,0u}));
                }
            }
            var actual=map.Parts.Enemies.Single(p=>p.EntityID==id);
            if(actual.NPCParamID!=original.NPCParamID||actual.ThinkParamID!=original.ThinkParamID||actual.ModelName!=original.ModelName)throw new Exception("First-act elite actor mismatch");
            PartyPlacement.Verify(actual);
            if(!map.Parts.Collisions.Any(c=>c.Name==actual.CollisionPartName)||!map.Models.Enemies.Any(m=>m.Name==actual.ModelName))throw new Exception("First-act elite model/collision missing");
            foreach(uint eventId in new[]{id,id+100000})PartyEnemyEvents.Verify(events.Events.Single(e=>e.ID==eventId),id);
        }
        init.Instructions.InsertRange(0,added);PartyModels.Prepare(map);
        foreach(var actor in actors)foreach(uint id in new[]{actor.EntityID,actor.EntityID+100000})
            if(init.Instructions.Count(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==id)!=1)throw new Exception("First-act elite event initialization mismatch");
        for(uint slot=0;slot<12;slot++)if(map.Regions.GetEntries().Count(r=>r.EntityID==PartyEnemyEvents.Region(48030294,slot))!=1)throw new Exception("First-act elite formation mismatch");
    }
    public static void Patch(string input,string output) {
        var manifest=File.ReadAllLines(Path.Combine(input,"arenas-resources.txt")).ToList();
        foreach(var row in File.ReadLines(Path.Combine(input,"arenas-built.txt")).Where(l=>l.Length>0&&!l.StartsWith('#'))) {
            var p=row.Split('|');if(p[10]!="1"&&p[0]!="farum_dragonlord")continue;
            var mp=$"map/mapstudio/{p[1]}.msb.dcx";var ep=$"event/{p[1]}.emevd.dcx";
            var map=MSBE.Read(Path.Combine(input,mp));var ev=EMEVD.Read(Path.Combine(input,ep));
            var originalMap=map.Write();var originalEvent=ev.Write();
            var oldParts=map.Parts.Enemies.Select(p=>p.Name).ToHashSet();var oldModels=map.Models.Enemies.Select(m=>m.Name).ToHashSet();
            var oldRegions=map.Regions.GetEntries().Select(r=>r.Name).ToHashSet();var oldEvents=ev.Events.Select(e=>e.ID).ToHashSet();
            int initCount=ev.Events.Single(e=>e.ID==0).Instructions.Count;Add(input,map,ev);
            var restoredMap=MSBE.Read(map.Write());var restoredEvent=EMEVD.Read(ev.Write());
            restoredMap.Parts.Enemies.RemoveAll(p=>!oldParts.Contains(p.Name));restoredMap.Models.Enemies.RemoveAll(m=>!oldModels.Contains(m.Name));
            restoredMap.Regions.InvasionPoints.RemoveAll(r=>!oldRegions.Contains(r.Name));restoredEvent.Events.RemoveAll(e=>!oldEvents.Contains(e.ID));
            var restoredInit=restoredEvent.Events.Single(e=>e.ID==0);restoredInit.Instructions.RemoveRange(0,restoredInit.Instructions.Count-initCount);
            if(!originalMap.SequenceEqual(restoredMap.Write())||!originalEvent.SequenceEqual(restoredEvent.Write()))throw new Exception("Unrelated first-act map/event data changed");
            foreach(var relative in new[]{mp,ep})Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,relative))!);
            map.Write(Path.Combine(output,mp));ev.Write(Path.Combine(output,ep));
            var checkMap=MSBE.Read(Path.Combine(output,mp));var checkEvent=EMEVD.Read(Path.Combine(output,ep));
            var beforeMap=checkMap.Write();var beforeEvent=checkEvent.Write();Add(input,checkMap,checkEvent);
            if(!beforeMap.SequenceEqual(checkMap.Write())||!beforeEvent.SequenceEqual(checkEvent.Write()))throw new Exception("First-act elite patch is not idempotent");
            foreach(var relative in new[]{mp,ep}) {
                var bytes=File.ReadAllBytes(Path.Combine(output,relative));ulong hash=14695981039346656037;foreach(byte b in bytes)hash=unchecked((hash^b)*1099511628211);
                int index=manifest.FindIndex(l=>l.StartsWith(relative+"|"));if(index<0)throw new Exception("Untracked arena file");manifest[index]=$"{relative}|{bytes.Length}|{hash:x16}";
            }
            Console.WriteLine($"Verified new first-act elite actors: {p[1]}");
        }
        File.WriteAllLines(Path.Combine(output,"arenas-resources.txt"),manifest);
    }
}
