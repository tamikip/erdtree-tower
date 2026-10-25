using SoulsFormats;

static class PartyGuardAssets {
    const uint Combat=45007997;
    static uint Offset(uint original)=>(original/10000-4801)*300+original%10000;
    static uint Position(uint n)=>n<1800?45012000+n:45022000+n-1800;
    static EMEVD.Event Convert(uint original,bool battle) {
        uint n=Offset(original),id=original+3000000;
        var ev=battle?PartyEnemyEvents.Battle(original):PartyEnemyEvents.Create(original);
        ev.ID+=3000000;
        var values=new Dictionary<uint,uint>{{original,id},{PartyEnemyEvents.Request(original),45003000+n},
            {PartyEnemyEvents.Ready(original),45002000+n},{PartyEnemyEvents.Combat,Combat}};
        for(uint bit=0;bit<4;bit++)values.Add(PartyEnemyEvents.Position(original)+bit,Position(n*4+bit));
        for(uint slot=0;slot<12;slot++)values.Add(PartyEnemyEvents.Region(original,slot),51059000+slot);
        foreach(var ins in ev.Instructions)for(int at=0;at+4<=ins.ArgData.Length;at+=4)
            if(values.TryGetValue(BitConverter.ToUInt32(ins.ArgData,at),out uint value))BitConverter.GetBytes(value).CopyTo(ins.ArgData,at);
        if(!battle) {
            // Complete backread removal before acknowledging reset and accepting a new wave.
            int clear=ev.Instructions.FindIndex(i=>i.Bank==2003&&i.ID==66&&i.ArgData[8]==0);
            var reset=ev.Instructions[clear];ev.Instructions.RemoveAt(clear);
            int unload=ev.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==29&&i.ArgData[4]==1);
            ev.Instructions.Insert(unload+1,new(4,7,new object[]{(sbyte)0,id,(byte)0,(sbyte)0,1f}));
            ev.Instructions.Insert(unload+2,reset);
            int endClear=ev.Instructions.FindLastIndex(i=>i.Bank==2003&&i.ID==66&&i.ArgData[8]==0);
            ev.Instructions.RemoveAt(endClear);
        }
        return ev;
    }
    public static void Build(string input,string output) {
        const string name="m45_01_00_00";
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name+".msb.dcx"));
        var events=EMEVD.Read(Path.Combine(input,"event",name+".emevd.dcx"));
        var init=events.Events.Single(e=>e.ID==0);
        var collision=map.Parts.Enemies.Single(p=>p.EntityID==45010100).CollisionPartName;
        var points=map.Regions.GetEntries().Where(r=>r.EntityID>=48029000&&r.EntityID<48029012).ToArray();
        if(points.Length!=12)throw new Exception("Guard formation source missing");
        foreach(var source in points) {
            var point=source.DeepCopy();point.EntityID=51059000+source.EntityID-48029000;point.Name=$"tower_guard_position_{point.EntityID}";
            if(map.Regions.GetEntries().Any(p=>p.EntityID==point.EntityID))throw new Exception("Guard region collision");
            map.Regions.Add(point);
        }
        foreach(var ins in events.Events.SelectMany(e=>e.Instructions))for(int at=0;at+4<=ins.ArgData.Length;at+=4) {
            uint v=BitConverter.ToUInt32(ins.ArgData,at);
            if(v==Combat||v>=45003000&&v<45003900||v>=45002000&&v<45002900||v>=45012000&&v<45013800||v>=45022000&&v<45023800)throw new Exception("Guard control range collision");
        }
        int count=0;
        for(int act=1;act<=3;act++) {
            var source=MSBE.Read(Path.Combine(input,"map/mapstudio",$"m45_0{3-act}_00_00.msb.dcx"));
            uint first=(uint)(48000000+act*10000);
            foreach(var template in source.Parts.Enemies.Where(p=>p.EntityID>=first&&p.EntityID<first+300)) {
                uint original=template.EntityID,id=original+3000000;
                if(map.Parts.GetEntries().Any(p=>p.EntityID==id))throw new Exception("Guard entity collision");
                var part=(MSBE.Part.Enemy)template.DeepCopy();part.Name=$"tower_guard_{id}";part.EntityID=id;
                part.InstanceID=12000+(int)Offset(original);part.CollisionPartName=collision;
                PartyPlacement.Prepare(part);
                if(!map.Models.Enemies.Any(m=>m.Name==part.ModelName))map.Models.Enemies.Add(new MSBE.Model.Enemy{Name=part.ModelName});
                map.Parts.Enemies.Add(part);
                foreach(bool battle in new[]{false,true}) {
                    var ev=Convert(original,battle);
                    if(events.Events.Any(e=>e.ID==ev.ID))throw new Exception("Guard event collision");
                    events.Events.Add(ev);init.Instructions.Insert(0,new(2000,0,new object[]{0,(uint)ev.ID,0u}));
                }
                init.Instructions.Insert(0,new(2003,66,new object[]{(byte)0,45003000+Offset(original),(byte)0}));
                init.Instructions.Insert(0,new(2004,5,new object[]{id,(byte)0}));count++;
            }
        }
        if(count!=684)throw new Exception($"Incomplete event guard pool: {count}");
        init.Instructions.Insert(0,new(2003,66,new object[]{(byte)0,Combat,(byte)0}));
        PartyModels.Prepare(map);PartyModels.Verify(map);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,"map/mapstudio",name+".msb.dcx"));events.Write(Path.Combine(output,"event",name+".emevd.dcx"));
        Verify(output);Console.WriteLine($"Verified {count} map guards across all three act pools");
    }
    public static void PatchFlags(string input,string output) {
        const string relative="event/m45_01_00_00.emevd.dcx";
        var events=EMEVD.Read(Path.Combine(input,relative));
        bool Target(uint v)=>v>=45002000&&v<45002900||v>=45012000&&v<45013800||v>=45022000&&v<45023800;
        foreach(var ev in events.Events.Where(e=>e.ID<51010000||e.ID>=51040000))
            foreach(var ins in ev.Instructions.Where(i=>i.Bank==3&&i.ID==0||i.Bank==2003&&i.ID==66))
                if(ins.ArgData.Length>=8&&Target(BitConverter.ToUInt32(ins.ArgData,4)))throw new Exception($"Guard flag collision in {ev.ID}");
        int changed=0;
        foreach(var ev in events.Events.Where(e=>e.ID>=51010000&&e.ID<51040000))
            foreach(var ins in ev.Instructions)for(int at=0;at+4<=ins.ArgData.Length;at+=4) {
                uint v=BitConverter.ToUInt32(ins.ArgData,at),next=v;
                if(v>=45006000&&v<45006900)next=v-4000;
                if(v>=45030000&&v<45033600)next=Position(v-45030000);
                if(next!=v){BitConverter.GetBytes(next).CopyTo(ins.ArgData,at);changed++;}
            }
        if(changed==0)throw new Exception("No legacy guard flags found");
        var target=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(target)!);events.Write(target);
        var checkedEvents=EMEVD.Read(target);
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_01_00_00.msb.dcx"));
        int verified=0;
        foreach(var part in map.Parts.Enemies.Where(p=>p.EntityID>=51010000&&p.EntityID<51040000))foreach(bool battle in new[]{false,true}) {
            var expected=Convert(part.EntityID-3000000,battle);var actual=checkedEvents.Events.Single(e=>e.ID==expected.ID);
            if(actual.Instructions.Count!=expected.Instructions.Count||actual.Instructions.Zip(expected.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception($"Migrated event mismatch: {actual.ID}");
            verified++;
        }
        if(verified!=1368)throw new Exception("Incomplete guard event migration");
        Console.WriteLine($"Migrated {changed} guard flag operands");
    }
    public static void Verify(string root) {
        var map=MSBE.Read(Path.Combine(root,"map/mapstudio/m45_01_00_00.msb.dcx"));
        var events=EMEVD.Read(Path.Combine(root,"event/m45_01_00_00.emevd.dcx"));
        var guards=map.Parts.Enemies.Where(p=>p.EntityID>=51010000&&p.EntityID<51040000).ToArray();
        if(guards.Length!=684)throw new Exception("Event room lacks shared guard entities");
        foreach(var part in guards) {
            PartyPlacement.Verify(part);
            if(part.TalkID!=-1||part.ChrActivateCondParamID!=0)throw new Exception("Unexpected guard activation rule");
            foreach(bool battle in new[]{false,true}) {
                var expected=Convert(part.EntityID-3000000,battle);var actual=events.Events.Single(e=>e.ID==expected.ID);PersonalEvents.Verify(actual);
                if(actual.Instructions.Count!=expected.Instructions.Count||actual.Instructions.Zip(expected.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Guard event mismatch");
            }
            var load=events.Events.Single(e=>e.ID==part.EntityID);
            int unload=load.Instructions.FindIndex(i=>i.Bank==4&&i.ID==7&&i.ArgData[8]==0);
            int reset=load.Instructions.FindIndex(i=>i.Bank==2003&&i.ID==66&&i.ArgData[8]==0);
            if(unload<0||reset<=unload)throw new Exception("Guard reset must wait for full unload");
        }
    }
}
