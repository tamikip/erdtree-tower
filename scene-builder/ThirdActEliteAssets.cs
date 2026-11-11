using SoulsFormats;

static class ThirdActEliteAssets {
    public static void ScaleNew(string input,string output) {
        var path=Path.Combine(output,"regulation.bin");var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam");
        var p=PARAM.Read(file.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml"));
        var tiers=TowerScalingAssets.Tiers(input)[3];
        foreach(var row in p.Rows.Where(r=>r.ID==950000109||r.ID>=950000318&&r.ID<=950000321))TowerScalingAssets.Apply(row,tiers[row.ID%2==1?1:2]);
        file.Bytes=p.Write();SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
    }
    static EMEVD.Event Lightning(uint entity,long id) {
        var ev=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);
        // Original Fortissax proximity lightning event 12032861, with only its actor replaced.
        foreach(var (bank,cmd,hex) in new (int,int,string)[]{
            (1003,1,"0101020000000000"),(2000,2,"00000000"),
            (4,5,"01000000204E0000323A0000010000000000803F"),(4,5,"01000000204E0000333A0000000000000000803F"),
            (3,3,"010100008293B700204E00000000704101000000"),(4,5,"010000008293B700303A0000010000000000803F"),
            (4,0,"010000008293B700000000000000803F"),(0,0,"00010100"),(1001,2,"0000000000004040"),
            (1004,1,"00000000204E0000323A00000000000001000000"),(2004,8,"204E0000333A0000"),
            (2003,5,"8293B7008293B700840300006B040000000000000000000000000000"),
            (1001,2,"000000400000A040"),(1014,0,""),(1000,4,"01000000")}) {
            var data=Convert.FromHexString(hex);
            for(int i=0;i+4<=data.Length;i+=4)if(BitConverter.ToUInt32(data,i)==12030850)BitConverter.GetBytes(entity).CopyTo(data,i);
            ev.Instructions.Add(new(bank,cmd,data));
        }
        return ev;
    }
    public static void Add(string input,MSBE map,EMEVD events) {
        var source=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_00_00_00.msb.dcx"));
        var early=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_02_00_00.msb.dcx"));
        var actors=source.Parts.Enemies.Where(e=>e.EntityID>=48030000&&e.EntityID<48030300&&e.NPCParamID>=950000318&&e.NPCParamID<=950000321).Select(p=>(source,p))
            .Concat(early.Parts.Enemies.Where(e=>e.EntityID>=48010000&&e.EntityID<48010300&&e.NPCParamID==950000109).Select(p=>(source:early,p))).ToArray();
        if(actors.Length!=5)throw new Exception("Expected two new bosses with elite variants and Death Rite Bird");
        var anchor=map.Parts.Enemies.First(p=>p.EntityID>=48030000&&p.EntityID<48030300);
        var init=events.Events.Single(e=>e.ID==0);var added=new List<EMEVD.Instruction>();
        foreach(var (origin,original) in actors) {
            var id=original.EntityID;
            if(!map.Parts.Enemies.Any(p=>p.EntityID==id)) {
                var part=(MSBE.Part.Enemy)original.DeepCopy();part.Name="tower_party_actor_"+id;
                part.Position=anchor.Position;part.Rotation=anchor.Rotation;part.CollisionPartName=anchor.CollisionPartName;
                part.WalkRouteName=null;part.PlatoonID=-1;
                part.InstanceID=map.Parts.Enemies.Where(p=>p.ModelName==part.ModelName).Select(p=>p.InstanceID).DefaultIfEmpty(-1).Max()+1;
                PartyPlacement.Prepare(part);
                if(!map.Models.Enemies.Any(m=>m.Name==part.ModelName))map.Models.Enemies.Add((MSBE.Model.Enemy)origin.Models.Enemies.Single(m=>m.Name==part.ModelName).DeepCopy());
                map.Parts.Enemies.Add(part);
                added.Add(new(2004,5,new object[]{id,(byte)0}));
                foreach(var flag in new[]{PartyEnemyEvents.Request(id),PartyEnemyEvents.Ready(id)})added.Add(new(2003,66,new object[]{(byte)0,flag,(byte)0}));
                foreach(var ev in new[]{PartyEnemyEvents.Create(id),PartyEnemyEvents.Battle(id)}) {
                    if(events.Events.Any(e=>e.ID==ev.ID))throw new Exception("Elite event collision");
                    events.Events.Add(ev);added.Add(new(2000,0,new object[]{0,(uint)ev.ID,0u}));
                }
            }
            var actual=map.Parts.Enemies.Single(p=>p.EntityID==id);
            if(actual.NPCParamID!=original.NPCParamID||actual.ThinkParamID!=original.ThinkParamID||actual.ModelName!=original.ModelName)throw new Exception("Elite actor mismatch");
        }
        for(uint slot=0;slot<12;slot++) {
            uint id=PartyEnemyEvents.Region(48010000,slot);
            if(map.Regions.GetEntries().Any(r=>r.EntityID==id))continue;
            var region=map.Regions.GetEntries().Single(r=>r.EntityID==PartyEnemyEvents.Region(48030000,slot)).DeepCopy();
            region.EntityID=id;region.Name="tower_act3_extra_position_"+slot;region.ActivationPartName=null;map.Regions.Add(region);
        }
        uint[] dragons=actors.Where(a=>a.p.ModelName=="c4511").Select(a=>a.p.EntityID).Concat(new uint[]{45000850,45000851,45000852}).ToArray();
        for(int slot=0;slot<dragons.Length;slot++) {
            var ev=Lightning(dragons[slot],96083000+slot);
            var existing=events.Events.SingleOrDefault(e=>e.ID==ev.ID);
            if(existing!=null) {
                if(existing.Instructions.Count!=ev.Instructions.Count||existing.Instructions.Zip(ev.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Fortissax event collision");
                continue;
            }
            events.Events.Add(ev);added.Add(new(2000,0,new object[]{0,(uint)ev.ID,0u}));
        }
        init.Instructions.InsertRange(0,added);PartyModels.Prepare(map);
        foreach(var (_,part) in actors)foreach(uint id in new[]{part.EntityID,part.EntityID+100000})PartyEnemyEvents.Verify(events.Events.Single(e=>e.ID==id),part.EntityID);
    }
    public static void Patch(string input,string output) {
        var manifest=File.ReadAllLines(Path.Combine(input,"arenas-resources.txt")).ToList();
        foreach(var line in File.ReadLines(Path.Combine(input,"arenas-built.txt")).Where(l=>l.Length>0&&!l.StartsWith('#'))) {
            var p=line.Split('|');if(p[10]!="3"&&p[0]!="farum_dragonlord")continue;
            var mp=$"map/mapstudio/{p[1]}.msb.dcx";var ep=$"event/{p[1]}.emevd.dcx";
            var map=MSBE.Read(Path.Combine(input,mp));var events=EMEVD.Read(Path.Combine(input,ep));Add(input,map,events);
            foreach(var relative in new[]{mp,ep})Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,relative))!);
            map.Write(Path.Combine(output,mp));events.Write(Path.Combine(output,ep));
            foreach(var relative in new[]{mp,ep}) {
                var bytes=File.ReadAllBytes(Path.Combine(output,relative));ulong hash=14695981039346656037;foreach(byte b in bytes)hash=unchecked((hash^b)*1099511628211);
                int index=manifest.FindIndex(l=>l.StartsWith(relative+"|"));if(index<0)throw new Exception("Untracked arena file");manifest[index]=$"{relative}|{bytes.Length}|{hash:x16}";
            }
            Console.WriteLine($"Third-act elite actors and Fortissax lightning verified: {p[1]}");
        }
        File.WriteAllLines(Path.Combine(output,"arenas-resources.txt"),manifest);
    }
}
