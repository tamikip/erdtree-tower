using SoulsFormats;

static class SecondActEliteAssets {
    static readonly uint[] Ids=[48010213,48010221];
    public static bool Add(string input,MSBE map,EMEVD events) {
        var source=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_02_00_00.msb.dcx"));
        var anchor=map.Parts.Enemies.First(p=>p.EntityID>=48020000&&p.EntityID<48020300);
        var added=new List<EMEVD.Instruction>();bool changed=false;
        foreach(uint id in Ids) {
            var original=source.Parts.Enemies.Single(p=>p.EntityID==id);
            var existing=map.Parts.Enemies.SingleOrDefault(p=>p.EntityID==id);
            if(existing!=null) {
                if(existing.NPCParamID!=original.NPCParamID||existing.ModelName!=original.ModelName)throw new Exception("Elite entity collision");
                continue;
            }
            var part=(MSBE.Part.Enemy)original.DeepCopy();
            part.Name="tower_party_actor_"+id;part.Position=anchor.Position;part.Rotation=anchor.Rotation;
            part.CollisionPartName=anchor.CollisionPartName;part.WalkRouteName=null;part.PlatoonID=-1;
            part.InstanceID=map.Parts.Enemies.Where(e=>e.ModelName==part.ModelName).Select(e=>e.InstanceID).DefaultIfEmpty(-1).Max()+1;
            PartyPlacement.Prepare(part);
            if(!map.Models.Enemies.Any(m=>m.Name==part.ModelName))map.Models.Enemies.Add((MSBE.Model.Enemy)source.Models.Enemies.Single(m=>m.Name==part.ModelName).DeepCopy());
            map.Parts.Enemies.Add(part);
            added.Add(new(2004,5,new object[]{id,(byte)0}));
            foreach(var flag in new[]{PartyEnemyEvents.Request(id),PartyEnemyEvents.Ready(id)})added.Add(new(2003,66,new object[]{(byte)0,flag,(byte)0}));
            foreach(var ev in new[]{PartyEnemyEvents.Create(id),PartyEnemyEvents.Battle(id)}) {
                if(events.Events.Any(e=>e.ID==ev.ID))throw new Exception("Elite event collision");
                events.Events.Add(ev);added.Add(new(2000,0,new object[]{0,(uint)ev.ID,0u}));
            }
            changed=true;
        }
        if(!changed)return false;
        for(uint slot=0;slot<12;slot++) {
            uint id=PartyEnemyEvents.Region(Ids[0],slot);
            if(map.Regions.GetEntries().Any(r=>r.EntityID==id))continue;
            var region=map.Regions.GetEntries().Single(r=>r.EntityID==PartyEnemyEvents.Region(48020000,slot)).DeepCopy();
            region.EntityID=id;region.Name="tower_act2_extra_position_"+slot;region.ActivationPartName=null;map.Regions.Add(region);
        }
        events.Events.Single(e=>e.ID==0).Instructions.InsertRange(0,added);
        PartyModels.Prepare(map);Verify(map,events);return true;
    }
    static void Verify(MSBE map,EMEVD events) {
        foreach(var id in Ids) {
            var part=map.Parts.Enemies.Single(p=>p.EntityID==id);PartyPlacement.Verify(part);
            if(!map.Parts.Collisions.Any(c=>c.Name==part.CollisionPartName))throw new Exception("Elite collision missing");
            foreach(var eventId in new[]{id,id+100000})PartyEnemyEvents.Verify(events.Events.Single(e=>e.ID==eventId),id);
        }
        for(uint slot=0;slot<12;slot++)if(map.Regions.GetEntries().Count(r=>r.EntityID==PartyEnemyEvents.Region(Ids[0],slot))!=1)throw new Exception("Elite position missing");
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var manifest=File.ReadAllLines(Path.Combine(input,"arenas-resources.txt")).ToList();
        foreach(var line in File.ReadLines(Path.Combine(input,"arenas-built.txt")).Where(l=>l.Length>0&&!l.StartsWith('#'))) {
            var p=line.Split('|');if(p[10]!="2"&&p[0]!="farum_dragonlord")continue;
            var mapPath=$"map/mapstudio/{p[1]}.msb.dcx";var eventPath=$"event/{p[1]}.emevd.dcx";
            var map=MSBE.Read(Path.Combine(input,mapPath));var events=EMEVD.Read(Path.Combine(input,eventPath));
            if(!Add(input,map,events)){Console.WriteLine($"Already present: {p[1]}");continue;}
            foreach(var path in new[]{mapPath,eventPath})Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,path))!);
            map.Write(Path.Combine(output,mapPath));events.Write(Path.Combine(output,eventPath));
            Verify(MSBE.Read(Path.Combine(output,mapPath)),EMEVD.Read(Path.Combine(output,eventPath)));
            foreach(var path in new[]{mapPath,eventPath}) {
                var bytes=File.ReadAllBytes(Path.Combine(output,path));ulong hash=14695981039346656037;
                foreach(byte value in bytes)hash=unchecked((hash^value)*1099511628211);
                int index=manifest.FindIndex(l=>l.StartsWith(path+"|"));if(index<0)throw new Exception("Resource not tracked: "+path);
                manifest[index]=$"{path}|{bytes.Length}|{hash:x16}";
            }
            Console.WriteLine($"Added and verified both elite actors: {p[1]}");
        }
        Directory.CreateDirectory(output);File.WriteAllLines(Path.Combine(output,"arenas-resources.txt"),manifest);
    }
}
