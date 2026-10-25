using SoulsFormats;
using System.Numerics;
using System.Text.Json;
using System.Globalization;

static class PartyEnemyAssets {
    record Template(int Act,int Model,int Npc,int Think,int Copies,bool Found,string SourceMap,string SourcePart);
    public static void Build(string input,string output) {
        var rows=JsonSerializer.Deserialize<Template[]>(File.ReadAllText("work/party-entity-audit/party-enemy-templates.json"));
        var fields=File.ReadAllLines(Path.Combine(input,"battlefields.txt")).Select(l=>l.Split('|')).ToArray();
        var manifest=new List<string>();
        var sources=new Dictionary<string,MSBE>();
        for(int act=1;act<=3;act++) {
            var field=fields.Single(f=>int.Parse(f[0])==act);
            string name=$"m45_0{3-act}_00_00";
            var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name+".msb.dcx"));
            var events=EMEVD.Read(Path.Combine(input,"event",name+".emevd.dcx"));
            foreach(var ev in events.Events)foreach(var ins in ev.Instructions)for(int at=0;at+4<=ins.ArgData.Length;at+=4) {
                uint value=BitConverter.ToUInt32(ins.ArgData,at);
                if(value==PartyEnemyEvents.Combat||value>=45008000&&value<45010000||value>=45004000+(act-1)*10000&&value<45005200+(act-1)*10000)throw new Exception($"Party control range already referenced: event={ev.ID}, instruction={ins.Bank}[{ins.ID}], value={value}");
            }
            uint basis=uint.Parse(field[2]);
            var stone=map.Parts.Enemies.Single(p=>p.EntityID==basis+100);
            Vector3 center=Vector3.Zero;
            var ids=map.Parts.GetEntries().Select(p=>p.EntityID).ToHashSet();
            float yaw=0;
            var entry=map.Regions.GetEntries().Single(r=>r.EntityID==basis+2600);
            for(uint slot=0;slot<12;slot++) {
                var point=entry.DeepCopy();point.EntityID=PartyEnemyEvents.Region((uint)(48000000+act*10000),slot);
                if(map.Regions.GetEntries().Any(r=>r.EntityID==point.EntityID)||ids.Contains(point.EntityID))throw new Exception("Party formation region collision");
                point.Name=$"tower_party_position_{slot}";point.ActivationPartName=null;
                float angle=MathF.Tau*((int)slot-1)/11;var local=slot==0?Vector3.Zero:new Vector3(MathF.Cos(angle)*6,0,MathF.Sin(angle)*6);
                point.Position=center+new Vector3(local.X*MathF.Cos(yaw)+local.Z*MathF.Sin(yaw),0.05f,-local.X*MathF.Sin(yaw)+local.Z*MathF.Cos(yaw));
                point.Rotation=new Vector3(0,MathF.Atan2(point.Position.X-entry.Position.X,point.Position.Z-entry.Position.Z)*180/MathF.PI,0);
                map.Regions.Add(point);
            }
            int ordinal=0;var added=new List<MSBE.Part.Enemy>();
            foreach(var row in rows.Where(r=>r.Act==act).OrderBy(r=>r.Npc)) {
                MSBE.Part.Enemy template;
                if(row.Found) {
                    if(!sources.TryGetValue(row.SourceMap,out var source)) {
                        source=MSBE.Read(Path.Combine("work/boss-report/maps",row.SourceMap+".msb.dcx"));sources.Add(row.SourceMap,source);
                    }
                    template=source.Parts.Enemies.Single(p=>p.Name==row.SourcePart);
                } else {
                    if(row.Model!=4341)throw new Exception($"Unreviewed missing template: {row.Model}");
                    template=new MSBE.Part.Enemy();
                }
                string model=$"c{row.Model:D4}";
                if(!map.Models.GetEntries().Any(m=>m.Name==model))map.Models.Enemies.Add(new MSBE.Model.Enemy{Name=model});
                for(int slot=0;slot<row.Copies;slot++) {
                    uint id=(uint)(48000000+act*10000+ordinal);
                    if(!ids.Add(id))throw new Exception($"Party enemy entity collision: {id}");
                    var part=(MSBE.Part.Enemy)template.DeepCopy();
                    part.Name=$"tower_party_{act}_{ordinal}";part.InstanceID=6000+ordinal++;
                    if(map.Parts.Enemies.Any(e=>e.ModelName==model&&e.InstanceID==part.InstanceID))throw new Exception("Party enemy instance collision");
                    part.ModelName=model;part.EntityID=id;part.EntityGroupIDs=new uint[8];
                    ArcherAssets.Normalize(part);
                    part.NPCParamID=row.Npc;part.ThinkParamID=row.Think;part.TalkID=-1;part.PlatoonID=-1;
                    part.ChrActivateCondParamID=0;part.BackupEventAnimID=-1;part.WalkRouteName=null;
                    part.CollisionPartName=stone.CollisionPartName;
                    part.Position=center+new Vector3(slot*3,0.05f,0);part.Rotation=Vector3.Zero;
                    PartyPlacement.Prepare(part);
                    map.Parts.Enemies.Add(part);added.Add(part);
                    if(events.Events.Any(e=>e.ID==id))throw new Exception($"Party enemy event collision: {id}");
                    events.Events.Add(PartyEnemyEvents.Create(id));
                    if(events.Events.Any(e=>e.ID==id+100000))throw new Exception("Party battle event collision");
                    events.Events.Add(PartyEnemyEvents.Battle(id));
                    events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,id+100000,0u}));
                    events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,id,0u}));
                    events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2003,66,new object[]{(byte)0,PartyEnemyEvents.Request(id),(byte)0}));
                    events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2004,5,new object[]{id,(byte)0}));
                    manifest.Add($"{act}|{row.Npc}|{row.Model}|{row.Think}|{slot}|{id}|{name}");
                }
            }
            events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2003,66,new object[]{(byte)0,PartyEnemyEvents.Combat,(byte)0}));
            string maps=Path.Combine(output,"map/mapstudio"),scripts=Path.Combine(output,"event");
            Directory.CreateDirectory(maps);Directory.CreateDirectory(scripts);
            string mapPath=Path.Combine(maps,name+".msb.dcx"),eventPath=Path.Combine(scripts,name+".emevd.dcx");
            PartyModels.Prepare(map);PartyModels.Verify(map);
            map.Write(mapPath);events.Write(eventPath);
            var reread=MSBE.Read(mapPath);var rereadEvents=EMEVD.Read(eventPath);var init=rereadEvents.Events.Single(e=>e.ID==0);
            var points=Enumerable.Range(0,12).Select(slot=>reread.Regions.GetEntries().Single(r=>r.EntityID==PartyEnemyEvents.Region((uint)(48000000+act*10000),(uint)slot))).ToArray();
            for(int slot=0;slot<points.Length;slot++) {
                if(!float.IsFinite(points[slot].Rotation.Y)||Vector3.Distance(points[slot].Position,center)>6.1f
                    ||points.Take(slot).Any(p=>Vector3.DistanceSquared(p.Position,points[slot].Position)<0.01f))throw new Exception("Invalid or overlapping formation points");
            }
            if(Vector3.Distance(points[0].Position,center)>0.1f)throw new Exception("Solo enemy formation is not centered");
            foreach(var part in added) {
                PartyEnemyEvents.Verify(rereadEvents.Events.Single(e=>e.ID==part.EntityID),part.EntityID);
                PartyEnemyEvents.Verify(rereadEvents.Events.Single(e=>e.ID==part.EntityID+100000),part.EntityID);
                var actual=reread.Parts.Enemies.Single(e=>e.EntityID==part.EntityID);
                if(actual.ModelName!=part.ModelName||actual.NPCParamID!=part.NPCParamID||actual.ThinkParamID!=part.ThinkParamID
                    ||actual.ChrActivateCondParamID!=0||actual.TalkID!=-1||actual.EntityGroupIDs.Any(id=>id!=0)
                    ||!init.Instructions.Any(i=>i.Bank==2004&&i.ID==5&&BitConverter.ToUInt32(i.ArgData,0)==part.EntityID&&i.ArgData[4]==0))throw new Exception("Party enemy roundtrip mismatch");
            }
            Console.WriteLine($"Verified act {act}: {added.Count} map-native enemy slots, all disabled at map initialization");
        }
        File.WriteAllLines(Path.Combine(output,"party-enemy-slots.txt"),manifest);
    }
}
