using SoulsFormats;
using System.Numerics;

static class ArenaChariotAssets {
    const string Map="m45_01_00_00";
    const uint Car=45011880,Chest=45011881,Portal=45011882,Entry=45012880;
    const uint Spawn=45007881,Loaded=45007882,Won=45007883,Request=45007884,Claimed=45007885,Exit=45007886;
    static void Flag(EMEVD.Event e,uint id,bool on)=>e.Instructions.Add(new(2003,66,new object[]{(byte)0,id,(byte)(on?1:0)}));
    static void Wait(EMEVD.Event e,uint id,bool on)=>e.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)(on?1:0),(byte)0,id}));
    static void ResetOnLoad(EMEVD events) {
        var init=events.Events.Single(e=>e.ID==0);
        for(uint flag=45007880;flag<=45007886;flag++) {
            init.Instructions.RemoveAll(i=>i.Bank==2003&&i.ID==66&&i.ArgData.Length>=12&&BitConverter.ToUInt32(i.ArgData,4)==flag);
            init.Instructions.Insert(0,new(2003,66,new object[]{(byte)0,flag,(byte)0}));
        }
    }
    public static void Build(string input,string output) {
        var map=MSBE.Read(Path.Combine(input,$"map/mapstudio/{Map}.msb.dcx"));
        var events=EMEVD.Read(Path.Combine(input,$"event/{Map}.emevd.dcx"));
        var source=MSBE.Read("work/boss-report/maps/m30_09_00_00.msb.dcx");
        foreach(var ins in events.Events.SelectMany(e=>e.Instructions))for(int i=0;i+4<=ins.ArgData.Length;i+=4) {
            uint value=BitConverter.ToUInt32(ins.ArgData,i);
            if(value>=45007880&&value<=45007886)throw new Exception("Chariot control flag collision");
        }
        if(map.Parts.GetEntries().Any(p=>p.EntityID>=Car&&p.EntityID<=Portal)||map.Regions.GetEntries().Any(p=>p.EntityID==Entry))throw new Exception("Arena chariot IDs already occupied");
        var original=source.Parts.Enemies.Single(e=>e.EntityID==30090400);
        var car=(MSBE.Part.Enemy)original.DeepCopy();car.Name="tower_arena_chariot";car.EntityID=Car;
        car.Position=new(0,.1f,0);car.Rotation=new(0,90,0);car.InstanceID=1880;
        car.CollisionPartName=map.Parts.Enemies.Single(e=>e.EntityID==45010100).CollisionPartName;
        car.EntityGroupIDs=new uint[8];car.BackupEventAnimID=-1;car.TalkID=-1;car.PlatoonID=-1;car.ChrActivateCondParamID=0;
        PartyPlacement.Prepare(car);
        var route=(MSBE.Event.PatrolInfo)source.Events.PatrolInfo.Single(p=>p.Name==original.WalkRouteName).DeepCopy();
        route.Name="tower_chariot_route";route.EntityID=45013880;route.EventID=map.Events.PatrolInfo.Select(p=>p.EventID).DefaultIfEmpty(0).Max()+1;
        route.WalkRegionNames=new string[64];
        for(int i=0;i<2;i++) {
            var region=source.Regions.GetEntries().Single(r=>r.Name==source.Events.PatrolInfo.Single(p=>p.Name==original.WalkRouteName).WalkRegionNames[i]).DeepCopy();
            region.Name=$"tower_chariot_route_{i}";region.EntityID=45012881u+(uint)i;region.Position=new(i==0?18:-18,.1f,0);region.ActivationPartName=null;
            map.Regions.Add(region);route.WalkRegionNames[i]=region.Name;
        }
        car.WalkRouteName=route.Name;map.Events.PatrolInfo.Add(route);map.Parts.Enemies.Add(car);
        if(!map.Models.Enemies.Any(m=>m.Name==car.ModelName))map.Models.Enemies.Add((MSBE.Model.Enemy)source.Models.Enemies.Single(m=>m.Name==car.ModelName).DeepCopy());
        var arrival=map.Regions.GetEntries().Single(r=>r.EntityID==45012600).DeepCopy();
        arrival.Name="tower_chariot_arrival";arrival.EntityID=Entry;arrival.Position=new(0,.2f,0);arrival.Rotation=Vector3.Zero;arrival.ActivationPartName=null;map.Regions.Add(arrival);
        void Asset(uint donor,uint id,Vector3 pos) {
            var p=(MSBE.Part.Asset)map.Parts.Assets.Single(a=>a.EntityID==donor).DeepCopy();p.Name=$"tower_chariot_{id}";p.EntityID=id;p.Position=pos;p.Rotation=Vector3.Zero;
            p.InstanceID=map.Parts.Assets.Where(a=>a.ModelName==p.ModelName).Max(a=>a.InstanceID)+1;p.EntityGroupIDs=new uint[8];p.UnkPartNames=new string[6];p.UnkT54PartName=null;
            map.Parts.Assets.Add(p);
        }
        Asset(45011901,Chest,Vector3.Zero);Asset(45011900,Portal,new(0,0,5));
        void Install(EMEVD.Event e){if(events.Events.Any(x=>x.ID==e.ID))throw new Exception("Chariot event collision");events.Events.Add(e);events.Events.Single(x=>x.ID==0).Instructions.Insert(0,new(2000,0,new object[]{0,(uint)e.ID,0u}));}
        EMEVD.Event Local(uint id){var e=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(e);return e;}
        var actor=Local(45017880);Flag(actor,Loaded,false);
        actor.Instructions.Add(new(2004,5,new object[]{Car,(byte)0}));actor.Instructions.Add(new(2004,29,new object[]{Car,(byte)1}));
        Wait(actor,Spawn,true);actor.Instructions.Add(new(2004,35,new object[]{Car,(byte)1}));actor.Instructions.Add(new(2004,29,new object[]{Car,(byte)0}));actor.Instructions.Add(new(2004,5,new object[]{Car,(byte)1}));
        actor.Instructions.Add(new(4,7,new object[]{(sbyte)0,Car,(byte)1,(sbyte)0,1f}));
        actor.Instructions.Add(new(2004,42,new object[]{Car,(byte)1,Entry,-1,45010100u}));
        actor.Instructions.Add(new(2004,15,new object[]{Car,(byte)1}));
        actor.Instructions.Add(new(2004,50,new object[]{Car,220,(byte)0}));
        actor.Instructions.Add(new(2004,30,new object[]{Car,(byte)0}));
        actor.Instructions.Add(new(2004,8,new object[]{Car,5000}));
        actor.Instructions.Add(new(2004,34,new object[]{Car,(byte)1}));
        actor.Instructions.Add(new(2004,1,new object[]{Car,(byte)1}));Flag(actor,Loaded,true);
        Wait(actor,Spawn,false);actor.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(actor);
        var visual=Local(45017881);visual.Instructions.Add(new(2005,3,new object[]{Chest,(byte)0}));
        Wait(visual,Won,true);visual.Instructions.Add(new(2005,1,new object[]{Chest,0}));visual.Instructions.Add(new(2005,3,new object[]{Chest,(byte)1}));
        Wait(visual,Won,false);visual.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(visual);
        var portalVisual=Local(45017884);portalVisual.Instructions.Add(new(2005,3,new object[]{Portal,(byte)0}));
        Wait(portalVisual,Won,true);Wait(portalVisual,Claimed,true);
        portalVisual.Instructions.Add(new(2005,1,new object[]{Chest,1}));
        portalVisual.Instructions.Add(new(2005,3,new object[]{Portal,(byte)1}));portalVisual.Instructions.Add(new(2006,4,new object[]{Portal,200,0xC4FD6}));
        Wait(portalVisual,Won,false);portalVisual.Instructions.Add(new(2006,5,new object[]{Portal,1}));portalVisual.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(portalVisual);
        var open=Local(45017882);Wait(open,Won,true);
        open.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)0,(byte)0,Claimed}));
        open.Instructions.Add(new(3,24,new object[]{(sbyte)1,9500002,Chest}));
        open.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
        open.Instructions.Add(new(2004,14,new object[]{10000u,Chest,60080,(byte)0}));
        open.Instructions.Add(new(2003,18,new object[]{Chest,1,(byte)0,(byte)1,(byte)0,(sbyte)0,1f}));Flag(open,Request,true);
        Wait(open,Won,false);open.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(open);
        var exit=Local(45017883);Wait(exit,Claimed,true);exit.Instructions.Add(new(3,24,new object[]{(sbyte)0,9500000,Portal}));Flag(exit,Exit,true);
        Wait(exit,Won,false);exit.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(exit);
        ResetOnLoad(events);PartyModels.Prepare(map);PartyModels.Verify(map);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,$"map/mapstudio/{Map}.msb.dcx"));events.Write(Path.Combine(output,$"event/{Map}.emevd.dcx"));
        var check=MSBE.Read(Path.Combine(output,$"map/mapstudio/{Map}.msb.dcx"));
        if(check.Parts.Enemies.Single(p=>p.EntityID==Car).WalkRouteName!=route.Name||check.Events.PatrolInfo.Single(p=>p.Name==route.Name).WalkRegionNames.Count(n=>n!=null)!=2)throw new Exception("Chariot patrol roundtrip mismatch");
        foreach(var e in EMEVD.Read(Path.Combine(output,$"event/{Map}.emevd.dcx")).Events.Where(e=>e.ID>=45017880&&e.ID<=45017884))PersonalEvents.Verify(e);
        Console.WriteLine("Arena chariot: native AI and reversible patrol, center spawn, personal victory chest and portal verified");
    }
    public static void PatchLoading(string input,string output) {
        var map=MSBE.Read(Path.Combine(input,$"map/mapstudio/{Map}.msb.dcx"));
        var events=EMEVD.Read(Path.Combine(input,$"event/{Map}.emevd.dcx"));
        var car=map.Parts.Enemies.Single(p=>p.EntityID==Car);
        ResetOnLoad(events);
        car.TalkID=-1;car.PlatoonID=-1;car.ChrActivateCondParamID=0;
        var actor=events.Events.Single(e=>e.ID==45017880);
        int load=actor.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==29&&i.ArgData[4]==0);
        if(load<0)throw new Exception("Chariot load event missing");
        if(!actor.Instructions.Any(i=>i.Bank==2004&&i.ID==35))actor.Instructions.Insert(load,new(2004,35,new object[]{Car,(byte)1}));
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,$"map/mapstudio/{Map}.msb.dcx"));events.Write(Path.Combine(output,$"event/{Map}.emevd.dcx"));
        var checkedEvent=EMEVD.Read(Path.Combine(output,$"event/{Map}.emevd.dcx")).Events.Single(e=>e.ID==45017880);
        int enable=checkedEvent.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==35);
        if(enable<0||checkedEvent.Instructions[enable+1].ID!=29||checkedEvent.Instructions[enable+1].ArgData[4]!=0)throw new Exception("Default backread must precede load request");
        PersonalEvents.Verify(checkedEvent);
        Console.WriteLine("Chariot default backread enabled before load; native template platoon/talk restrictions cleared; event roundtrip verified");
    }
}
