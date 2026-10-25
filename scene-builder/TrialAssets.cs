using SoulsFormats;
using System.Numerics;

static class TrialAssets {
    const uint Active=45007990, Crossed=45007991;
    static void Flag(EMEVD.Event e,uint id,bool value)=>e.Instructions.Add(new(2003,66,new object[]{(byte)0,id,(byte)(value?1:0)}));
    static void Wait(EMEVD.Event e,uint id,bool value)=>e.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)(value?1:0),(byte)0,id}));
    static EMEVD.Event Local(uint id){var e=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(e);return e;}
    static void Install(EMEVD f,EMEVD.Event e){if(f.Events.Any(x=>x.ID==e.ID))throw new Exception("Trial event collision");f.Events.Add(e);f.Events.Single(x=>x.ID==0).Instructions.Insert(0,new(2000,0,new object[]{0,(uint)e.ID,0u}));}
    static void Asset(MSBE map,MSBE source,uint donor,uint id,Vector3 pos,Vector3 rot) {
        var original=source.Parts.Assets.Single(p=>p.EntityID==donor);
        CopyAsset(map,source,original,id,pos,rot);
    }
    static MSBE.Part.Asset CopyAsset(MSBE map,MSBE source,MSBE.Part.Asset original,uint id,Vector3 pos,Vector3 rot) {
        var p=(MSBE.Part.Asset)original.DeepCopy();p.Name="tower_trial_"+id;p.EntityID=id;p.Position=pos;p.Rotation=rot;
        p.InstanceID=map.Parts.Assets.Where(a=>a.ModelName==p.ModelName).Select(a=>a.InstanceID).DefaultIfEmpty(-1).Max()+1;
        p.EntityGroupIDs=new uint[8];p.UnkPartNames=new string[6];p.UnkT54PartName=original.UnkT54PartName==original.Name?p.Name:null;
        p.MapStudioLayer=uint.MaxValue;p.Unk1.DisplayGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();p.Unk1.DrawGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();
        if(!map.Models.Assets.Any(m=>m.Name==p.ModelName))map.Models.Assets.Add((MSBE.Model.Asset)source.Models.Assets.Single(m=>m.Name==p.ModelName).DeepCopy());
        if(map.Parts.GetEntries().Any(x=>x.EntityID==id))throw new Exception("Trial asset collision");map.Parts.Assets.Add(p);return p;
    }
    static void Point(MSBE map,uint id,Vector3 pos,float yaw) {
        if(map.Regions.GetEntries().Any(x=>x.EntityID==id))throw new Exception("Trial region collision");
        map.Regions.Others.Add(new MSBE.Region.Other{Name="tower_trial_"+id,EntityID=id,Position=pos,Rotation=new Vector3(0,yaw,0),Shape=new MSB.Shape.Sphere{Radius=2}});
    }
    static void DescentArrival(MSBE map,uint id,Vector3 pos,float yaw) {
        if(map.Regions.GetEntries().Any(r=>r.EntityID==id))throw new Exception("Descent arrival collision");
        var point=(MSBE.Region.SpawnPoint)map.Regions.GetEntries().Single(r=>r.EntityID==35002840).DeepCopy();
        point.Name="tower_trial_"+id;point.EntityID=id;point.Position=pos;point.Rotation=new(0,yaw,0);point.ActivationPartName=null;
        map.Regions.Add(point);
    }
    static void DescentPlayers(MSBE map) {
        var template=map.Parts.Players.Single(p=>p.EntityID==35000980);
        foreach(uint regionId in new uint[]{35002980,35002981}) {
            var region=map.Regions.GetEntries().Single(r=>r.EntityID==regionId);
            uint playerId=regionId-1992;
            var player=map.Parts.Players.SingleOrDefault(p=>p.EntityID==playerId);
            if(player==null) {
                player=(MSBE.Part.Player)template.DeepCopy();player.Name="tower_descent_player_"+playerId;player.EntityID=playerId;
                map.Parts.Players.Add(player);
            }
            player.Position=region.Position;player.Rotation=region.Rotation;
        }
        var upper=map.Regions.GetEntries().Single(r=>r.EntityID==35002980);
        var fallback=map.Parts.Players.Single(p=>p.Name=="c0000_0000");
        fallback.Position=upper.Position;fallback.Rotation=upper.Rotation;
    }
    public static void Build(string input,string output) {
        var source=MSBE.Read("work/descent-inspect/cache/m35_00_00_00.msb.dcx");
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        for(uint act=0;act<3;act++) {
            string name=$"m45_0{act}_00_00";uint basis=45000000+act*10000,gate=basis+1950,inside=basis+2950;
            var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name+".msb.dcx"));var events=EMEVD.Read(Path.Combine(input,"event",name+".emevd.dcx"));
            var doorModel=new[]{"AEG400_043","AEG400_143","AEG400_270"}[act];
            var door=map.Parts.Assets.Single(a=>a.ModelName==doorModel&&a.Position.X>0);
            if(door.EntityID!=0)throw new Exception("Arena door already has an event owner");
            var threshold=new Vector3(door.Position.X,0.10f,door.Position.Z);
            float yaw=door.Rotation.Y-180;
            Asset(map,source,35001800,gate,threshold,new(0,yaw,0));
            map.Parts.Assets.Single(a=>a.EntityID==gate).Scale=new(1.18f,1.05f,1);
            float facing=door.Rotation.Y;
            Point(map,inside,new(22.3f,0.15f,-2.5f),facing);
            var arrival=map.Regions.GetEntries().Single(r=>r.EntityID==basis+2600).DeepCopy();
            arrival.Name="tower_combat_arrival";arrival.EntityID=basis+2951;arrival.ActivationPartName=null;
            arrival.Position=new(28.35f,0.35f,-2.66f);arrival.Rotation=new(0,facing,0);map.Regions.Add(arrival);
            Point(map,basis+2952,arrival.Position,facing);
            var attendant=map.Parts.Enemies.Single(e=>e.EntityID==basis+1810);
            attendant.Position=new(28.35f,0.35f,-4.06f);attendant.Rotation=new(0,facing,0);
            var visual=Local(basis+7950);
            visual.Instructions.Add(new(2006,5,new object[]{gate,1}));visual.Instructions.Add(new(2005,3,new object[]{gate,(byte)0}));
            Wait(visual,Active,true);visual.Instructions.Add(new(2005,3,new object[]{gate,(byte)1}));
            visual.Instructions.Add(new(2006,4,new object[]{gate,101,5}));Wait(visual,Active,false);visual.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(events,visual);
            var placement=Local(basis+7952);Wait(placement,Active,true);Wait(placement,45007992,true);
            placement.Instructions.Add(new(4,7,new object[]{(sbyte)0,10000u,(byte)1,(sbyte)0,1f}));
            placement.Instructions.Add(new(2004,42,new object[]{10000u,(byte)1,basis+2951,-1,basis+100}));
            placement.Instructions.Add(new(3,2,new object[]{(sbyte)0,(byte)1,10000u,basis+2952,1}));
            Flag(placement,45007993,true);Wait(placement,45007992,false);
            placement.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(events,placement);
            var rescue=Local(basis+7953);Wait(rescue,45007994,true);
            rescue.Instructions.Add(new(2004,42,new object[]{10000u,(byte)1,basis+2600,-1,basis+100}));
            Flag(rescue,45007994,false);rescue.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(events,rescue);
            var cross=Local(basis+7951);Flag(cross,Crossed,false);Wait(cross,Active,true);Wait(cross,45007993,true);
            cross.Instructions.Add(new(3,24,new object[]{(sbyte)0,10000,gate}));
            cross.Instructions.Add(new(2004,14,new object[]{10000u,inside,60060,(byte)1}));
            cross.Instructions.Add(new(2004,42,new object[]{10000u,(byte)1,inside,-1,basis+100}));
            cross.Instructions.Add(new(3,2,new object[]{(sbyte)0,(byte)1,10000u,inside,1}));
            Flag(cross,Crossed,true);Wait(cross,Active,false);cross.Instructions.Add(new(1000,4,new object[]{(byte)1}));Install(events,cross);
            events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new(2003,66,new object[]{(byte)0,Crossed,(byte)0}));
            map.Write(Path.Combine(output,"map/mapstudio",name+".msb.dcx"));events.Write(Path.Combine(output,"event",name+".emevd.dcx"));
            var check=EMEVD.Read(Path.Combine(output,"event",name+".emevd.dcx"));PersonalEvents.Verify(check.Events.Single(e=>e.ID==cross.ID));
            VerifyEntrance(MSBE.Read(Path.Combine(output,"map/mapstudio",name+".msb.dcx")),check,basis);
        }
        Console.WriteLine("Three local fog gates and traversal events verified");
        Descent(input,output,source);
    }
    public static void Verify(string input) {
        for(uint act=0;act<3;act++) {
            string name=$"m45_0{act}_00_00";
            VerifyEntrance(MSBE.Read(Path.Combine(input,"map/mapstudio",name+".msb.dcx")),EMEVD.Read(Path.Combine(input,"event",name+".emevd.dcx")),45000000+act*10000);
        }
    }
    public static void VerifyDescent(string input) {
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio/m35_00_00_00.msb.dcx"));
        foreach(uint id in new uint[]{35002980,35002981}) {
            var point=map.Regions.GetEntries().Single(r=>r.EntityID==id);
            if(point is not MSBE.Region.SpawnPoint)throw new Exception($"Descent arrival {id} must be a native SpawnPoint, got {point.GetType().Name}");
            var player=map.Parts.Players.SingleOrDefault(p=>p.EntityID==id-1992);
            if(player==null||player.Position!=point.Position||player.Rotation!=point.Rotation)throw new Exception($"Missing player arrival for descent point {id}");
        }
        if(map.Parts.Players.Single(p=>p.Name=="c0000_0000").Position!=map.Regions.GetEntries().Single(r=>r.EntityID==35002980).Position)throw new Exception("Default descent spawn must be at the upper platform");
        Console.WriteLine("Both descent arrivals are native spawn points");
        var events=EMEVD.Read(Path.Combine(input,"event/m35_00_00_00.emevd.dcx"));
        var native=EMEVD.Read("work/descent-inspect/cache/m35_00_00_00.emevd.dcx");
        if(events.Format!=native.Format||events.Compression.GetType()!=native.Compression.GetType())throw new Exception("Descent event format/compression must match native Elden Ring");
        if(!events.LinkedFileOffsets.SequenceEqual(native.LinkedFileOffsets)||!events.StringData.SequenceEqual(native.StringData))throw new Exception("Descent common-event dependencies must match native Elden Ring");
        Console.WriteLine("Descent event format, compression and common dependencies verified");
    }
    public static void PatchDescent(string input,string output) {
        const string relative="map/mapstudio/m35_00_00_00.msb.dcx";
        var map=MSBE.Read(Path.Combine(input,relative));
        foreach(uint id in new uint[]{35002980,35002981}) {
            var previous=map.Regions.GetEntries().Single(r=>r.EntityID==id);
            if(previous is MSBE.Region.SpawnPoint)continue;
            if(previous is not MSBE.Region.Other other)throw new Exception("Unexpected descent arrival type");
            map.Regions.Others.Remove(other);
            DescentArrival(map,id,previous.Position,previous.Rotation.Y);
        }
        DescentPlayers(map);
        var path=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(path)!);map.Write(path);
        var events=EMEVD.Read(Path.Combine(input,"event/m35_00_00_00.emevd.dcx"));
        var native=EMEVD.Read("work/descent-inspect/cache/m35_00_00_00.emevd.dcx");
        native.Events=events.Events;
        Directory.CreateDirectory(Path.Combine(output,"event"));
        native.Write(Path.Combine(output,"event/m35_00_00_00.emevd.dcx"));
        VerifyDescent(output);
    }
    static void VerifyEntrance(MSBE map,EMEVD events,uint basis) {
        var startup=events.Events.Single(e=>e.ID==0).Instructions.TakeWhile(i=>i.Bank>=2000).ToList();
        for(uint id=basis+7950;id<=basis+7953;id++)
            if(startup.Count(i=>i.Bank==2000&&i.ID==0&&i.ArgData.Length>=8&&BitConverter.ToUInt32(i.ArgData,4)==id)!=1)
                throw new Exception($"Fog initializer {id} must run once before arena conditions or termination");
        var gate=map.Parts.Assets.Single(a=>a.EntityID==basis+1950);
        if(map.Parts.Assets.Any(a=>a.EntityID==basis+1952))throw new Exception("Synthetic entrance floor must be absent");
        var outside=map.Regions.GetEntries().Single(r=>r.EntityID==basis+2951);
        var inside=map.Regions.GetEntries().Single(r=>r.EntityID==basis+2950);
        var service=map.Regions.GetEntries().Single(r=>r.EntityID==basis+2600);
        if(outside.GetType()!=service.GetType())throw new Exception("Combat spawn must use the native arrival region type");
        if(map.Parts.Enemies.Single(e=>e.EntityID==basis+1810).Position!=new Vector3(28.35f,0.35f,-4.06f))throw new Exception("Combat attendant is not inside the verified east vestibule");
        PersonalEvents.Verify(events.Events.Single(e=>e.ID==basis+7952));
        var door=map.Parts.Assets.Single(a=>a.Position.X>0&&a.ModelName==new[]{"AEG400_043","AEG400_143","AEG400_270"}[(basis-45000000)/10000]);
        if(outside.Position!=new Vector3(28.35f,0.35f,-2.66f)||inside.Position.X!=22.3f||service.Position.Z!=10)throw new Exception("Verified vestibule arrival mismatch");
        if(Math.Abs(gate.Position.X-door.Position.X)>.01f||Math.Abs(gate.Position.Z-door.Position.Z)>.01f||Math.Abs(gate.Rotation.Y-(door.Rotation.Y-180))>.01f||gate.Scale.X!=1.18f)throw new Exception("Fog does not match original east gate");
        if(Math.Abs(outside.Rotation.Y-door.Rotation.Y)>.01f||inside.Rotation!=outside.Rotation)throw new Exception("Arrival must face through the gate into arena");
        var placement=events.Events.Single(e=>e.ID==basis+7952).Instructions;
        if(!placement.Any(i=>i.Bank==3&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==Active))throw new Exception("Placement must require an active combat gate");
        var instructions=events.Events.Single(e=>e.ID==basis+7951).Instructions;
        int animation=instructions.FindIndex(i=>i.Bank==2004&&i.ID==14),warp=instructions.FindIndex(i=>i.Bank==2004&&i.ID==42);
        if(animation<0||warp<=animation||BitConverter.ToUInt32(instructions[warp].ArgData,8)!=basis+2950)throw new Exception("Fog traversal destination mismatch");
    }
    static void Descent(string input,string output,MSBE map) {
        const uint chest=35001980,portal=35001981;
        var upper=map.Parts.Enemies.Single(e=>e.EntityID==35000393).Position;
        var lower=map.Parts.Players.Single(e=>e.EntityID==35000984).Position;
        DescentArrival(map,35002980,upper+new Vector3(0,0.1f,0),142);
        DescentArrival(map,35002981,lower+new Vector3(0,0.1f,0),15);
        DescentPlayers(map);
        var donor=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_01_00_00.msb.dcx"));
        Asset(map,donor,45011901,chest,lower,new(0,15,0));
        var exitPosition=map.Parts.Enemies.Single(e=>e.EntityID==35000700).Position;
        Asset(map,donor,45011900,portal,exitPosition,new(0,195,0));
        map.Parts.Assets.RemoveAll(a=>a.EntityID==35001954);
        map.Events.Treasures.Clear();map.Events.ObjActs.Clear();
        var file=EMEVD.Read("work/descent-inspect/cache/m35_00_00_00.emevd.dcx");file.Events.Clear();
        var init=new EMEVD.Event(0,EMEVD.Event.RestBehaviorType.Default);file.Events.Add(init);
        uint anonymous=35006000;
        foreach(var enemy in map.Parts.Enemies) {
            if(enemy.EntityID==0)enemy.EntityID=anonymous++;
            init.Instructions.Add(new(2004,5,new object[]{enemy.EntityID,(byte)0}));
            init.Instructions.Add(new(2004,29,new object[]{enemy.EntityID,(byte)1}));
        }
        foreach(var id in new uint[]{35007991,35007992,35007993,35007994})Flag(init,id,false);
        var floor=Local(35007982);
        floor.Instructions.Add(new(3,2,new object[]{(sbyte)0,(byte)1,10000u,35002506u,1}));
        floor.Instructions.Add(new(1001,0,new object[]{0.15f}));
        floor.Instructions.Add(new(2005,1,new object[]{35001506u,1}));Install(file,floor);
        foreach(uint function in new uint[]{90005690,90005691})init.Instructions.Add(new(2000,6,new object[]{0,function,35002507u}));
        var open=Local(35007980);
        open.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,35007990u}));
        open.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)0,(byte)0,35007992u}));
        open.Instructions.Add(new(3,24,new object[]{(sbyte)1,9500002,chest}));
        open.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));Flag(open,35007994,true);
        open.Instructions.Add(new(2004,14,new object[]{10000u,chest,60080,(byte)0}));
        open.Instructions.Add(new(1001,0,new object[]{0.25f}));
        open.Instructions.Add(new(2003,18,new object[]{chest,1,(byte)0,(byte)1,(byte)0,(sbyte)0,1f}));Flag(open,35007991,true);
        Install(file,open);
        var exit=Local(35007981);Wait(exit,35007992,true);
        exit.Instructions.Add(new(2006,4,new object[]{portal,200,0xC4FD6}));
        exit.Instructions.Add(new(3,24,new object[]{(sbyte)0,9500000,portal}));Flag(exit,35007993,true);Install(file,exit);
        string path=Path.Combine(output,"map/mapstudio/m35_00_00_00.msb.dcx");map.Write(path);
        file.Write(Path.Combine(output,"event/m35_00_00_00.emevd.dcx"));
        var check=MSBE.Read(path);
        VerifyDescent(output);
        if(check.Events.Treasures.Count!=0||check.Regions.GetEntries().Count(r=>r.EntityID is 35002980 or 35002981)!=2)throw new Exception("Descent scene roundtrip failed");
        foreach(var e in EMEVD.Read(Path.Combine(output,"event/m35_00_00_00.emevd.dcx")).Events.Where(e=>e.ID!=0))PersonalEvents.Verify(e);
        File.WriteAllText(Path.Combine(output,"descent.txt"),$"{upper.X}|{upper.Y}|{upper.Z}|{lower.X}|{lower.Y}|{lower.Z}",System.Text.Encoding.UTF8);
        Console.WriteLine($"Descent anchors copied from native NPC/player entries: {upper} -> {lower}; original story events excluded");
    }
}
