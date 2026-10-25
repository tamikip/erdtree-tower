using SoulsFormats;

static class ChariotAssets {
    const string Map="m30_09_00_00";
    const uint Boss=30090800,Active=30097990;
    static bool BossEvent(uint id)=>id>=30092800&&id<30092900;
    public static void Build(string output) {
        var map=MSBE.Read($"work/boss-report/maps/{Map}.msb.dcx");
        var events=EMEVD.Read($"work/chariot-inspect/{Map}.emevd.dcx");
        var entry=map.Parts.Players.Single(p=>p.EntityID==30090980);
        var arrival=(MSBE.Part.Player)entry.DeepCopy();arrival.Name="tower_chariot_arrival";arrival.EntityID=30090988;
        map.Parts.Players.Add(arrival);
        foreach(var player in map.Parts.Players){player.Position=entry.Position;player.Rotation=entry.Rotation;}
        foreach(var e in events.Events)e.Instructions.RemoveAll(i=>i.Bank==2000&&(i.ID==0||i.ID==6)&&i.ArgData.Length>=8
            &&(BossEvent(BitConverter.ToUInt32(i.ArgData,4))||Enumerable.Range(2,i.ArgData.Length/4-2).Any(n=>BitConverter.ToUInt32(i.ArgData,n*4)==Boss)));
        events.Events.RemoveAll(e=>BossEvent((uint)e.ID));
        var setup=new EMEVD.Event(30097980,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(setup);
        setup.Instructions.Add(new(2004,5,new object[]{Boss,(byte)0}));
        setup.Instructions.Add(new(2004,29,new object[]{Boss,(byte)1}));
        setup.Instructions.Add(new(2006,4,new object[]{30091800u,101,5}));
        var cleanup=new EMEVD.Event(30097981,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(cleanup);
        cleanup.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,Active}));
        uint anonymous=30096000;
        foreach(var enemy in map.Parts.Enemies) {
            if(enemy.EntityID==0||enemy.EntityID==uint.MaxValue) {
                while(map.Parts.GetEntries().Any(p=>p.EntityID==anonymous))anonymous++;
                enemy.EntityID=anonymous++;
            }
            cleanup.Instructions.Add(new(2004,5,new object[]{enemy.EntityID,(byte)0}));
        }
        cleanup.Instructions.Add(new(1001,0,new object[]{0.2f}));
        cleanup.Instructions.Add(new(1000,4,new object[]{(byte)1}));
        var init=events.Events.Single(e=>e.ID==0);
        foreach(var ev in new[]{setup,cleanup}){events.Events.Add(ev);init.Instructions.Insert(0,new(2000,0,new object[]{0,(uint)ev.ID,0u}));}
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        map.Write(Path.Combine(output,$"map/mapstudio/{Map}.msb.dcx"));events.Write(Path.Combine(output,$"event/{Map}.emevd.dcx"));
        Verify(output);
    }
    public static void Verify(string output) {
        var map=MSBE.Read(Path.Combine(output,$"map/mapstudio/{Map}.msb.dcx"));
        var original=MSBE.Read($"work/boss-report/maps/{Map}.msb.dcx");
        var events=EMEVD.Read(Path.Combine(output,$"event/{Map}.emevd.dcx"));
        var native=EMEVD.Read($"work/chariot-inspect/{Map}.emevd.dcx");
        var entry=original.Parts.Players.Single(p=>p.EntityID==30090980);
        if(map.Parts.Players.Count(p=>p.EntityID==30090988)!=1||map.Parts.Players.Any(p=>p.Position!=entry.Position||p.Rotation!=entry.Rotation))throw new Exception("Chariot arrival mismatch");
        if(map.Parts.Enemies.Count!=original.Parts.Enemies.Count||map.Parts.Enemies.Zip(original.Parts.Enemies).Any(p=>p.First.Position!=p.Second.Position||p.First.NPCParamID!=p.Second.NPCParamID||p.First.ThinkParamID!=p.Second.ThinkParamID))throw new Exception("Chariot encounter changed");
        if(map.Parts.Enemies.Count(p=>p.ModelName=="c8120")!=2)throw new Exception("Expected two native chariots");
        foreach(long id in new long[]{30092399,30092400,30092410}) {
            var a=events.Events.Single(e=>e.ID==id);var b=native.Events.Single(e=>e.ID==id);
            if(a.Instructions.Count!=b.Instructions.Count||a.Instructions.Zip(b.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData))||a.Parameters.Count!=b.Parameters.Count)throw new Exception("Native chariot event changed");
        }
        if(events.Events.Any(e=>BossEvent((uint)e.ID)))throw new Exception("Boss events remain");
        foreach(uint id in new uint[]{30097980,30097981})PersonalEvents.Verify(events.Events.Single(e=>e.ID==id));
        if(events.Format!=native.Format||!events.StringData.SequenceEqual(native.StringData)||!events.LinkedFileOffsets.SequenceEqual(native.LinkedFileOffsets))throw new Exception("Native event dependencies changed");
        Console.WriteLine($"Verified chariot trial: native arrival {entry.Position}, two chariots, preserved route scripts, boss disabled");
    }
}
