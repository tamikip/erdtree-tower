using SoulsFormats;
using System.Numerics;

static class EventHostAssets {
    internal static string[][] Rows(string input) {
        var rows=File.ReadAllLines(Path.Combine(input,"event-hosts.txt")).Select(l=>l.Split('|')).ToArray();
        if(rows.Length!=9||rows.Any(r=>r.Length!=6)||new[]{0,1,2,4,5}.Any(i=>rows.Select(r=>r[i]).Distinct().Count()!=9)
            ||rows.Any(r=>uint.Parse(r[1])%10000>=4000||uint.Parse(r[4]) is <45011840 or >45011848))throw new Exception("Invalid event host entity range or visibility flags");
        return rows;
    }
    public static void Patch(string input,string output) {
        const string mapPath="map/mapstudio/m45_01_00_00.msb.dcx",eventPath="event/m45_01_00_00.emevd.dcx";
        var rows=Rows(input);
        var regulation=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var characters=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="CharaInitParam").Bytes);
        if(rows.Any(r=>!characters.Rows.Any(c=>c.ID==int.Parse(r[2]))))throw new Exception("Event character preset missing from regulation");
        var map=MSBE.Read(Path.Combine(input,mapPath));var events=EMEVD.Read(Path.Combine(input,eventPath));
        var donor=MSBE.Read(Path.Combine(input,"map/mapstudio/m11_10_00_00.msb.dcx")).Parts.Enemies.Single(e=>e.EntityID==45001800);
        var old=map.Parts.Enemies.Single(e=>e.EntityID==45011801);
        var init=events.Events.Single(e=>e.ID==0);
        foreach(var row in rows) {
            uint id=uint.Parse(row[1]),flag=uint.Parse(row[4]);
            if(map.Parts.GetEntries().Any(p=>p.EntityID==id)||events.Events.Any(e=>e.ID==id))throw new Exception($"Event host ID already present: {id}");
            foreach(var instruction in events.Events.SelectMany(e=>e.Instructions))for(int i=0;i+4<=instruction.ArgData.Length;i+=4)
                if(BitConverter.ToUInt32(instruction.ArgData,i)==flag)throw new Exception("Event host visibility flag collision");
            var part=(MSBE.Part.Enemy)donor.DeepCopy();part.Name=$"tower_event_host_{row[0]}";part.EntityID=id;
            part.InstanceID=map.Parts.Enemies.Where(e=>e.ModelName=="c0000").Select(e=>e.InstanceID).DefaultIfEmpty(9800).Max()+1;
            part.CharaInitID=int.Parse(row[2]);part.Position=new(0,.1f,0);part.Rotation=new(0,180,0);part.Scale=Vector3.One;
            if(row[0]=="Dice")part.ThinkParamID=523090010;
            part.CollisionPartName=old.CollisionPartName;part.TalkID=-1;part.EntityGroupIDs=new uint[8];part.WalkRouteName=null;part.BackupEventAnimID=-1;part.ChrActivateCondParamID=0;
            PartyPlacement.Prepare(part);map.Parts.Enemies.Add(part);
            if(!map.Models.GetEntries().Any(m=>m.Name=="c0000"))map.Models.Players.Add(new MSBE.Model.Player{Name="c0000"});
            events.Events.Add(PartyServiceAssets.Visibility(id,flag));
            init.Instructions.Insert(0,new(2000,0,new object[]{0,id,0u}));
            init.Instructions.Insert(0,new(2003,66,new object[]{(byte)0,flag,(byte)0}));
        }
        PartyModels.Prepare(map);PartyModels.Verify(map);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,mapPath));events.Write(Path.Combine(output,eventPath));
        var check=MSBE.Read(Path.Combine(output,mapPath));var checkedEvents=EMEVD.Read(Path.Combine(output,eventPath));
        foreach(var row in rows) {
            uint id=uint.Parse(row[1]);var npc=check.Parts.Enemies.Single(e=>e.EntityID==id);
            if(npc.ModelName!="c0000"||npc.NPCParamID!=523200079||npc.CharaInitID!=int.Parse(row[2])||npc.Position!=new Vector3(0,.1f,0))throw new Exception("Event host roundtrip mismatch");
            PersonalEvents.Verify(checkedEvents.Events.Single(e=>e.ID==id));PartyPlacement.Verify(npc);
        }
        Console.WriteLine("Verified nine central event NPCs with distinct character presets and local visibility events");
        EventHostNames.Patch(input,output,Path.Combine(output,mapPath));
    }
    static void Migrate(EMEVD file,string[][] rows) {
        var init=file.Events.Single(e=>e.ID==0);
        foreach(var row in rows) {
            uint id=uint.Parse(row[1]),flag=uint.Parse(row[4]),legacy=id+70;
            var host=file.Events.Single(e=>e.ID==id);
            var waits=host.Instructions.Where(i=>i.Bank==3&&i.ID==0).ToArray();
            var resets=init.Instructions.Where(i=>i.Bank==2003&&i.ID==66&&i.ArgData.Length>=9
                &&(BitConverter.ToUInt32(i.ArgData,4)==legacy||BitConverter.ToUInt32(i.ArgData,4)==flag)).ToArray();
            if(waits.Length!=2||waits.Any(i=>i.ArgData.Length!=8||(BitConverter.ToUInt32(i.ArgData,4)!=legacy&&BitConverter.ToUInt32(i.ArgData,4)!=flag))
                ||waits[0].ArgData[1]!=1||waits[1].ArgData[1]!=0||resets.Length!=1||resets[0].ArgData[8]!=0)
                throw new Exception($"Unexpected event host visibility script: {id}");
            var targets=waits.Concat(resets).ToHashSet();
            foreach(var ev in file.Events)foreach(var ins in ev.Instructions)for(int at=0;at+4<=ins.ArgData.Length;at++) {
                uint value=BitConverter.ToUInt32(ins.ArgData,at);
                if((value==flag||value==legacy)&&!(targets.Contains(ins)&&at==4))throw new Exception($"Visibility flag collision: {value} in event {ev.ID}");
            }
            foreach(var ins in targets)BitConverter.GetBytes(flag).CopyTo(ins.ArgData,4);
            PersonalEvents.Verify(host);
        }
    }
    public static void PatchVisibility(string input,string output) {
        const string path="event/m45_01_00_00.emevd.dcx";
        var file=EMEVD.Read(Path.Combine(input,path));Migrate(file,Rows(input));
        var target=Path.Combine(output,path);Directory.CreateDirectory(Path.GetDirectoryName(target)!);file.Write(target);
        var check=EMEVD.Read(target);var before=check.Write();Migrate(check,Rows(input));
        if(!before.SequenceEqual(check.Write()))throw new Exception("Event host visibility patch is not idempotent");
        Console.WriteLine("Verified nine event host flags after serialization; repeated migration leaves events unchanged");
    }
    static void RemapEntities(MSBE map,EMEVD file,string[][] rows,bool reverse=false) {
        for(int n=0;n<rows.Length;n++) {
            uint legacy=45016000+(uint)n,current=uint.Parse(rows[n][1]);
            uint from=reverse?current:legacy,to=reverse?legacy:current;
            if(!map.Parts.Enemies.Any(p=>p.EntityID==from)) {
                if(!map.Parts.Enemies.Any(p=>p.EntityID==to&&p.Name=="tower_event_host_"+rows[n][0]))throw new Exception("Missing event host");
                continue;
            }
            if(map.Parts.GetEntries().Any(p=>p.EntityID==to)||map.Regions.GetEntries().Any(r=>r.EntityID==to)||map.Events.GetEntries().Any(e=>e.EntityID==to)||file.Events.Any(e=>e.ID==to))throw new Exception($"Event host entity collision: {to}");
            var host=map.Parts.Enemies.Single(p=>p.EntityID==from);
            if(host.Name!="tower_event_host_"+rows[n][0]||host.CharaInitID!=int.Parse(rows[n][2]))throw new Exception("Unexpected event host template");
            int changed=0;
            foreach(var ev in file.Events)foreach(var ins in ev.Instructions)for(int at=0;at+4<=ins.ArgData.Length;at+=4) {
                uint value=BitConverter.ToUInt32(ins.ArgData,at);
                if(value==to)throw new Exception($"Event host instruction target collision: {to}");
                if(value!=from)continue;
                bool init=ev.ID==0&&ins.Bank==2000&&ins.ID==0&&at==4;
                bool actor=ev.ID==from&&ins.Bank==2004&&(ins.ID is 5 or 29 or 35)&&at==0;
                if(!init&&!actor)throw new Exception($"Unexpected event host reference: {from}");
                BitConverter.GetBytes(to).CopyTo(ins.ArgData,at);changed++;
            }
            if(changed!=6)throw new Exception($"Expected six character references, found {changed}");
            file.Events.Single(e=>e.ID==from).ID=to;host.EntityID=to;
        }
    }
    public static void PatchEntities(string input,string output) {
        const string mapPath="map/mapstudio/m45_01_00_00.msb.dcx",eventPath="event/m45_01_00_00.emevd.dcx";
        var rows=Rows(input);var map=MSBE.Read(Path.Combine(input,mapPath));var file=EMEVD.Read(Path.Combine(input,eventPath));
        var beforeMap=map.Write();var beforeEvents=file.Write();
        bool legacy=map.Parts.Enemies.Any(p=>p.EntityID==45016000);
        RemapEntities(map,file,rows);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,mapPath));file.Write(Path.Combine(output,eventPath));
        map=MSBE.Read(Path.Combine(output,mapPath));file=EMEVD.Read(Path.Combine(output,eventPath));
        foreach(var row in rows) {
            var npc=map.Parts.Enemies.Single(p=>p.EntityID==uint.Parse(row[1]));PartyPlacement.Verify(npc);
            if(npc.ModelName!="c0000"||(npc.NPCParamID!=523200079&&npc.NPCParamID!=EventHostNames.Npc(row))||npc.Position!=new Vector3(0,.1f,0))throw new Exception("Event host placement changed");
        }
        var afterMap=map.Write();var afterEvents=file.Write();
        RemapEntities(map,file,rows);Migrate(file,rows);
        if(!afterMap.SequenceEqual(map.Write())||!afterEvents.SequenceEqual(file.Write()))throw new Exception("Entity migration is not idempotent");
        if(legacy)RemapEntities(map,file,rows,true);
        if(!beforeMap.SequenceEqual(map.Write())||!beforeEvents.SequenceEqual(file.Write()))throw new Exception("Entity migration changed unrelated data");
        Console.WriteLine("Verified nine character IDs, 54 instruction targets, visibility gates, repeat migration and exact preservation of unrelated map/event data");
    }
    public static void TestVisibility(string input) {
        var rows=Rows(input);var file=new EMEVD();var init=new EMEVD.Event(0);file.Events.Add(init);
        init.Instructions.Add(new(2003,66,new object[]{(byte)0,45011870u,(byte)0}));
        var unrelated=new EMEVD.Event(900);unrelated.Instructions.Add(new(2003,66,new object[]{(byte)0,45010940u,(byte)1}));file.Events.Add(unrelated);
        foreach(var row in rows) {
            uint id=uint.Parse(row[1]);file.Events.Add(PartyServiceAssets.Visibility(id));
            init.Instructions.Add(new(2003,66,new object[]{(byte)0,id+70,(byte)0}));
        }
        var original=file.Write();Migrate(file,rows);file=EMEVD.Read(file.Write());
        foreach(var row in rows) {
            uint id=uint.Parse(row[1]),flag=uint.Parse(row[4]);
            var actual=file.Events.Single(e=>e.ID==id);var expected=PartyServiceAssets.Visibility(id,flag);
            if(actual.Instructions.Count!=expected.Instructions.Count||actual.Instructions.Zip(expected.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Visibility roundtrip mismatch");
        }
        var migrated=file.Write();Migrate(file,rows);if(!migrated.SequenceEqual(file.Write()))throw new Exception("Migration changed on repeat");
        foreach(var row in rows)foreach(var ev in file.Events)foreach(var ins in ev.Instructions.Where(i=>i.Bank==3&&i.ID==0||i.Bank==2003&&i.ID==66))
            if(BitConverter.ToUInt32(ins.ArgData,4)==uint.Parse(row[4]))BitConverter.GetBytes(uint.Parse(row[1])+70).CopyTo(ins.ArgData,4);
        if(!original.SequenceEqual(file.Write()))throw new Exception("Migration changed unrelated event data");
        file.Events.Single(e=>e.ID==900).Instructions.Add(new(2003,66,new object[]{(byte)0,45011840u,(byte)1}));
        bool rejected=false;try{Migrate(file,rows);}catch(Exception){rejected=true;}
        if(!rejected)throw new Exception("Visibility collision was accepted");
        PartyServiceAssets.TestLoading();
        Console.WriteLine("Passed event host migration, serialized flags, idempotence, unrelated-data preservation and collision rejection");
    }
}
