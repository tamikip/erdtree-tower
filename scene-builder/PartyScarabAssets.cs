using SoulsFormats;
using System.Numerics;

static class PartyScarabAssets {
    static uint Request(uint id)=>45007000+(id-48040000)*4;
    static EMEVD.Event Load(uint id) {
        var ev=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);
        uint request=Request(id);
        ev.Instructions.Add(new(2004,5,new object[]{id,(byte)0}));
        ev.Instructions.Add(new(2004,29,new object[]{id,(byte)1}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,request+1,(byte)0}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,request}));
        ev.Instructions.Add(new(2004,35,new object[]{id,(byte)1}));
        ev.Instructions.Add(new(2004,29,new object[]{id,(byte)0}));
        ev.Instructions.Add(new(4,7,new object[]{(sbyte)0,id,(byte)1,(sbyte)0,1f}));
        ev.Instructions.Add(new(2004,42,new object[]{id,(byte)1,48049000+(id-48040000)%3,-1,10000u}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,request+1,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,request}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));return ev;
    }
    static EMEVD.Event Visible(uint id) {
        var ev=new EMEVD.Event(id+100000,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);
        uint request=Request(id);
        ev.Instructions.Add(new(2004,5,new object[]{id,(byte)0}));
        foreach(uint flag in new[]{request,request+1,request+2})ev.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,flag}));
        ev.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
        ev.Instructions.Add(new(2004,5,new object[]{id,(byte)1}));
        foreach(uint flag in new[]{request,request+2})ev.Instructions.Add(new(3,0,new object[]{(sbyte)-1,(byte)0,(byte)0,flag}));
        ev.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)-1}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));return ev;
    }
    public static void Build(string input,string output) {
        const string name="m45_01_00_00";
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name+".msb.dcx"));
        var eventPath=Path.Combine(output,"event",name+".emevd.dcx");
        var events=EMEVD.Read(File.Exists(eventPath)?eventPath:Path.Combine(input,"event",name+".emevd.dcx"));
        foreach(var ins in events.Events.SelectMany(e=>e.Instructions))for(int at=0;at+4<=ins.ArgData.Length;at+=4) {
            uint value=BitConverter.ToUInt32(ins.ArgData,at);
            if(value>=45007000&&value<45007288)throw new Exception("Scarab control flag collision");
        }
        MSBE.Part.Enemy source=null;
        foreach(var path in Directory.EnumerateFiles("work/boss-report/maps","*.msb.dcx").Order()) {
            source=MSBE.Read(path).Parts.Enemies.FirstOrDefault(e=>e.ModelName=="c4191");
            if(source!=null){Console.WriteLine($"Scarab template: {Path.GetFileName(path)} / {source.Name}");break;}
        }
        if(source==null)throw new Exception("No original scarab map template");
        var collision=map.Parts.Enemies.Single(e=>e.EntityID==45010100).CollisionPartName;
        var region=map.Regions.GetEntries().Single(r=>r.EntityID==45012600);
        for(uint slot=0;slot<3;slot++) {
            var point=region.DeepCopy();point.EntityID=48049000+slot;point.Name=$"tower_scarab_position_{slot}";point.ActivationPartName=null;
            point.Position=new Vector3(-4+slot*4,.05f,0);point.Rotation=new(0,slot*120,0);
            if(map.Regions.GetEntries().Any(r=>r.EntityID==point.EntityID))throw new Exception("Scarab region collision");map.Regions.Add(point);
        }
        if(!map.Models.Enemies.Any(m=>m.Name=="c4191"))map.Models.Enemies.Add(new MSBE.Model.Enemy{Name="c4191"});
        var init=events.Events.Single(e=>e.ID==0);
        for(uint i=0;i<72;i++) {
            uint id=48040000+i;
            if(map.Parts.GetEntries().Any(p=>p.EntityID==id)||events.Events.Any(e=>e.ID==id||e.ID==id+100000))throw new Exception("Scarab entity collision");
            var part=(MSBE.Part.Enemy)source.DeepCopy();part.Name=$"tower_scarab_{i}";part.InstanceID=9800+(int)i;
            part.EntityID=id;part.EntityGroupIDs=new uint[8];part.NPCParamID=941912000;part.ThinkParamID=41912000;
            part.TalkID=-1;part.PlatoonID=-1;part.CharaInitID=-1;part.ChrActivateCondParamID=0;part.BackupEventAnimID=-1;part.WalkRouteName=null;
            part.CollisionPartName=collision;part.Position=new(-4+i%3*4,.05f,0);part.Rotation=new(0,i%3*120,0);PartyPlacement.Prepare(part);map.Parts.Enemies.Add(part);
            events.Events.Add(Load(id));events.Events.Add(Visible(id));
            foreach(uint ev in new[]{id,id+100000})init.Instructions.Insert(0,new(2000,0,new object[]{0,ev,0u}));
            foreach(uint flag in new[]{Request(id),Request(id)+1,Request(id)+2})init.Instructions.Insert(0,new(2003,66,new object[]{(byte)0,flag,(byte)0}));
            init.Instructions.Insert(0,new(2004,5,new object[]{id,(byte)0}));
        }
        PartyModels.Prepare(map);PartyModels.Verify(map);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        var mapPath=Path.Combine(output,"map/mapstudio",name+".msb.dcx");map.Write(mapPath);events.Write(eventPath);
        var checkedMap=MSBE.Read(mapPath);var checkedEvents=EMEVD.Read(eventPath);
        for(uint id=48040000;id<48040072;id++) {
            var part=checkedMap.Parts.Enemies.Single(p=>p.EntityID==id);PartyPlacement.Verify(part);
            if(part.NPCParamID!=941912000||part.ModelName!="c4191"||part.TalkID!=-1||part.CollisionPartName!=collision)throw new Exception("Scarab map roundtrip mismatch");
            foreach(var expected in new[]{Load(id),Visible(id)}) {
                var actual=checkedEvents.Events.Single(e=>e.ID==expected.ID);PersonalEvents.Verify(actual);
                if(actual.Instructions.Count!=expected.Instructions.Count||actual.Instructions.Zip(expected.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Scarab event roundtrip mismatch");
            }
        }
        Console.WriteLine("Verified 72 map scarabs, 24 distinct attempts and 144 local loading/visibility events");
    }
}
