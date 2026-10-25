using SoulsFormats;
using System.Numerics;

static class PartyCampAssets {
    public static void Build(string input,string output) {
        const string name="m45_00_00_00.msb.dcx";
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name));
        var eventName="m45_00_00_00.emevd.dcx";
        var events=EMEVD.Read(Path.Combine(input,"event",eventName));
        var hub=MSBE.Read("work/scene-cache/m11_10_00_00.msb.dcx");
        var human=hub.Parts.Enemies.Single(e=>e.EntityID==11100730);
        var smith=hub.Parts.Enemies.First(e=>e.ModelName=="c3451");
        var anchor=map.Parts.Enemies.Single(e=>e.EntityID==45000100);
        var collision=anchor.CollisionPartName??map.Parts.Collisions.Single(c=>c.Name=="h001000").Name;
        var positions=new[]{new Vector3(-5,.15f,5),new Vector3(5,.05f,1),new Vector3(5,.15f,6)};
        int[] parameters=[523200079,34510179,523180079],thoughts=[523200000,34519000,523180000],initial=[23200,-1,23180];
        int count=map.Parts.Enemies.Count;
        for(int slot=0;slot<3;slot++) {
            var part=(MSBE.Part.Enemy)(slot==1?smith:human).DeepCopy();
            uint id=45001800u+(uint)slot;
            if(map.Parts.GetEntries().Any(p=>p.EntityID==id))throw new Exception("Camp service entity collision");
            part.EntityID=id;part.Name=$"tower_camp_service_{slot}";part.InstanceID=9800+slot;
            if(map.Parts.Enemies.Any(p=>p.ModelName==part.ModelName&&p.InstanceID==part.InstanceID))throw new Exception("Camp service instance collision");
            part.NPCParamID=parameters[slot];part.ThinkParamID=thoughts[slot];part.CharaInitID=initial[slot];
            part.Position=positions[slot];part.Rotation=new(0,MathF.Atan2(part.Position.X,part.Position.Z-10)*180/MathF.PI,0);
            part.TalkID=-1;part.EntityGroupIDs=new uint[8];part.WalkRouteName=null;part.ChrActivateCondParamID=0;
            part.CollisionPartName=collision;part.BackupEventAnimID=-1;
            if(!map.Models.GetEntries().Any(m=>m.Name==part.ModelName)) {
                if(slot==1)map.Models.Enemies.Add(new MSBE.Model.Enemy{Name=part.ModelName});
                else map.Models.Players.Add(new MSBE.Model.Player{Name=part.ModelName});
            }
            PartyPlacement.Prepare(part);
            map.Parts.Enemies.Add(part);
            if(events.Events.Any(e=>e.ID==id))throw new Exception("Camp visibility event collision");
            events.Events.Add(PartyServiceAssets.Visibility(id));
            events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new(2000,0,new object[]{0,id,0u}));
            events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new(2003,66,new object[]{(byte)0,id+70,(byte)0}));
        }
        PartyModels.Prepare(map);PartyModels.Verify(map);
        string directory=Path.Combine(output,"map/mapstudio");Directory.CreateDirectory(directory);
        string path=Path.Combine(directory,name);map.Write(path);var check=MSBE.Read(path);
        if(check.Parts.Enemies.Count!=count+3)throw new Exception("Camp service count mismatch");
        for(int slot=0;slot<3;slot++) {
            var part=check.Parts.Enemies.Single(e=>e.EntityID==45001800u+slot);
            PartyPlacement.Verify(part);
            if(part.NPCParamID!=parameters[slot]||part.ThinkParamID!=thoughts[slot]||part.CharaInitID!=initial[slot]
                ||part.Position!=positions[slot]||part.TalkID!=-1||part.ChrActivateCondParamID!=0||part.EntityGroupIDs.Any(v=>v!=0)
                ||!check.Parts.Collisions.Any(c=>c.Name==part.CollisionPartName)||!check.Models.GetEntries().Any(m=>m.Name==part.ModelName))throw new Exception($"Camp service roundtrip mismatch: slot={slot} npc={part.NPCParamID} think={part.ThinkParamID} init={part.CharaInitID} pos={part.Position} talk={part.TalkID} activation={part.ChrActivateCondParamID} collision={part.CollisionPartName} model={part.ModelName} groups={string.Join(',',part.EntityGroupIDs)}");
        }
        Console.WriteLine("Verified 3 map-native camp services, original entities retained");
        Directory.CreateDirectory(Path.Combine(output,"event"));
        var eventPath=Path.Combine(output,"event",eventName);events.Write(eventPath);
        var checkedEvents=EMEVD.Read(eventPath);
        for(uint id=45001800;id<=45001802;id++)PersonalEvents.Verify(checkedEvents.Events.Single(e=>e.ID==id));
    }
}
