using SoulsFormats;
using System.Numerics;

static class RegionalAssets {
    public static void BlastRegions(MSBE map) {
        for(uint slot=0;slot<3;slot++)for(uint point=0;point<9;point++) {
            uint id=45019800+slot*10+point;
            if(map.Regions.GetEntries().Any(r=>r.EntityID==id))throw new Exception("Chest blast region collision");
            var angle=((int)point-1)*MathF.PI/4;
            var offset=point==0?Vector3.Zero:new Vector3(3*MathF.Cos(angle),0,3*MathF.Sin(angle));
            map.Regions.Others.Add(new MSBE.Region.Other{Name=$"TowerChestBlast_{slot}_{point}",EntityID=id,Position=new Vector3(((int)slot-1)*5,0.15f,0)+offset,Shape=new MSB.Shape.Point()});
        }
    }
    static EMEVD.Instruction I(int bank,int id,params object[] args)=>new(bank,id,args);
    static EMEVD.Instruction Flag(sbyte group,uint flag,bool enabled)=>I(3,0,group,(byte)(enabled?1:0),(byte)0,flag);
    static EMEVD.Instruction Set(uint flag,bool enabled)=>I(2003,66,(byte)0,flag,(byte)(enabled?1:0));
    static EMEVD.Instruction Restart()=>I(1000,4,(byte)1);
    public static void Build(EMEVD events) {
        void Add(uint id,params EMEVD.Instruction[] instructions) {
            if(events.Events.Any(e=>e.ID==id))throw new Exception($"Regional event collision: {id}");
            var ev=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);ev.Instructions.AddRange(instructions);events.Events.Add(ev);
            events.Events.Single(e=>e.ID==0).Instructions.Insert(0,I(2000,0,0,id,0u));
        }
        Add(45010800,PersonalChestEvents.VisibilityInstructions());
        Add(45010840,Flag(1,45010940,true),Flag(1,45010942,false),I(0,0,(sbyte)0,(byte)1,(sbyte)1),
            I(2006,4,45011900u,200,0xC4FD6),Flag(-1,45010940,false),Flag(-1,45010942,true),I(0,0,(sbyte)0,(byte)1,(sbyte)-1),I(2006,5,45011900u,1),Restart());
        Add(45010841,Flag(1,45010940,true),Flag(1,45010960,true),Flag(1,45010963,false),
            I(3,24,(sbyte)1,9500003,45011911u),I(0,0,(sbyte)0,(byte)1,(sbyte)1),Set(45010961,true),I(1001,0,1f),Restart());
        Add(45010842,Flag(1,45010940,true),Flag(1,45010960,true),Flag(1,45010962,true),I(0,0,(sbyte)0,(byte)1,(sbyte)1),
            I(2004,14,10000u,45011911u,60080,(byte)0),I(1001,0,0.25f),I(2003,18,45011911u,1,(byte)0,(byte)1,(byte)0,(sbyte)0,1f),
            Flag(0,45010940,false),Restart());
        Add(45010843,Flag(1,45010940,true),Flag(1,45010960,true),Flag(1,45010963,true),Flag(1,45010962,false),I(0,0,(sbyte)0,(byte)1,(sbyte)1),
            I(2005,7,45011911u,1),Flag(0,45010940,false),Restart());
        for(uint slot=0;slot<3;slot++) {
            uint box=45011910+slot,opened=45010950+slot,request=45010953+slot;
            var visibility=new List<EMEVD.Instruction>{I(2005,3,box,(byte)0),Flag(1,45010940,true),Flag(1,45010941,false)};
            if(slot!=1)visibility.Add(Flag(1,45010960,false));
            visibility.AddRange(new[]{I(0,0,(sbyte)0,(byte)1,(sbyte)1),I(2005,7,box,0),I(2005,3,box,(byte)1),Flag(-1,45010940,false),Flag(-1,45010941,true)});
            if(slot!=1)visibility.Add(Flag(-1,45010960,true));
            visibility.AddRange(new[]{I(0,0,(sbyte)0,(byte)1,(sbyte)-1),Restart()});Add(45010801+slot,visibility.ToArray());
            Add(45010811+slot,
                Flag(1,45010940,true),Flag(1,45010941,false),Flag(1,45010960,false),Flag(1,45010942,false),Flag(1,opened,false),
                I(3,24,(sbyte)1,9500002,box),I(0,0,(sbyte)0,(byte)1,(sbyte)1),
                I(2004,14,10000u,box,60080,(byte)0),I(1001,0,0.25f),I(2003,18,box,1,(byte)0,(byte)1,(byte)0,(sbyte)0,1f),
                Set(request,true),I(1001,0,1f),Restart());
            var blast=new List<EMEVD.Instruction>{Flag(0,45010943+slot,true)};
            for(uint point=0;point<9;point++)blast.Add(I(2006,3,1,45019800+slot*10+point,-1,300122));
            blast.AddRange(new[]{Set(45010943+slot,false),Restart()});Add(45010821+slot,blast.ToArray());
            Add(45010831+slot,Flag(1,45010940,true),Flag(1,45010960,false),Flag(1,opened,true),I(0,0,(sbyte)0,(byte)1,(sbyte)1),
                I(2003,18,box,1,(byte)0,(byte)1,(byte)0,(sbyte)0,1f),Flag(0,45010940,false),Restart());
        }
    }
    public static void Verify(string output) {
        var map=MSBE.Read(Path.Combine(output,"map","mapstudio","m45_01_00_00.msb.dcx"));
        var events=EMEVD.Read(Path.Combine(output,"event","m45_01_00_00.emevd.dcx"));
        if(events.Events.Select(e=>e.ID).Distinct().Count()!=events.Events.Count)throw new Exception("Duplicate event IDs");
        foreach(var ev in events.Events.Where(e=>e.ID>=45010800&&e.ID<=45010843))PersonalEvents.Verify(ev);
        foreach(var instruction in events.Events.Where(e=>e.ID>=45010800&&e.ID<=45010843).SelectMany(e=>e.Instructions)) {
            int length=(instruction.Bank,instruction.ID) switch {
                (2000,2)=>4,(3,0)=>8,(3,24)=>12,(0,0)=>4,(2005,3)=>8,(2005,7)=>8,(2004,14)=>16,(2003,18)=>16,
                (2003,66)=>12,(2006,3)=>16,(2006,4)=>12,(2006,5)=>8,(1001,0)=>4,(1000,4)=>4,_=>throw new Exception("Unexpected regional instruction")
            };
            if(instruction.ArgData.Length!=length)throw new Exception("Regional instruction layout mismatch");
        }
        if(!events.Events.Single(e=>e.ID==45010841).Instructions.Any(i=>i.Bank==3&&i.ID==24&&BitConverter.ToInt32(i.ArgData,4)==9500003))throw new Exception("Locked chest action missing");
        if(!events.Events.Single(e=>e.ID==45010842).Instructions.Any(i=>i.Bank==3&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==45010962))throw new Exception("Locked chest must await successful key transaction");
        for(uint slot=0;slot<3;slot++) {
            if(map.Parts.Assets.Count(a=>a.EntityID==45011910+slot)!=1)throw new Exception("Event chest missing");
            if(!events.Events.Single(e=>e.ID==45010811+slot).Instructions.Any(i=>i.Bank==2004&&i.ID==14&&BitConverter.ToInt32(i.ArgData,8)==60080))throw new Exception("Event chest must use native lid-opening animation");
            if(events.Events.Single(e=>e.ID==45010821+slot).Instructions.Count(i=>i.Bank==2006&&i.ID==3)!=9)throw new Exception("Chest blast must cover its expanded ring");
            for(uint point=0;point<9;point++) {
                var region=map.Regions.GetEntries().Single(r=>r.EntityID==45019800+slot*10+point);
                float radius=Vector2.Distance(new Vector2(region.Position.X,region.Position.Z),new Vector2(((int)slot-1)*5,0));
                if(Math.Abs(radius-(point==0?0:3))>0.001f)throw new Exception("Chest blast ring misplaced");
            }
            foreach(uint prefix in new uint[]{45010801,45010811,45010821,45010831})if(events.Events.Count(e=>e.ID==prefix+slot)!=1)throw new Exception("Event chest lifecycle missing");
        }
        Console.WriteLine("Verified three regional chests, original explosion SFX and event activation gates");
    }
}
