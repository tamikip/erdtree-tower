using SoulsFormats;
using System.Numerics;

static class DragonlordEntrance {
    const uint Basis=45020000,Active=45007990,Crossed=45007991,Gate=Basis+1950;
    const uint CountdownSfx=Basis+2960,Placed=45007992;
    public static Vector3 WorldPosition(uint id,Func<Vector3,Vector3> place)=>id switch {
        // 传送门与其他战斗场地一样放在场地正中，战斗中关闭、胜利后出现（PortalReveal）
        Basis+1900=>place(new Vector3(0,0.03f,0)),
        Basis+1810=>new(15.993273f,1009.634827f,346.978882f),
        _=>place(Position(id)),
    };
    public static Vector3 Position(uint id)=>id switch {
        Basis+100=>new(-10,0.1f,0),
        Basis+1810=>new(29,0.35f,-4),
        Basis+1900=>new(24,0.03f,5),
        Gate=>new(25,0.03f,0),
        Basis+2950=>new(0,0.15f,0),
        _=>new(30,0.35f,0),
    };
    static void Flag(EMEVD.Event e,uint id,bool value)=>e.Instructions.Add(new(2003,66,new object[]{(byte)0,id,(byte)(value?1:0)}));
    static void Wait(EMEVD.Event e,uint id,bool value)=>e.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)(value?1:0),(byte)0,id}));
    static EMEVD.Event Local(uint id){var e=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(e);return e;}
    public static void Apply(MSBE map,EMEVD events) {
        var gate=map.Parts.Assets.Single(a=>a.EntityID==Gate);
        map.Parts.Assets.Remove(gate);
        map.Parts.Assets.Single(a=>a.EntityID==Basis+1900).Rotation=new(0,178.23f,0);
        map.Regions.GetEntries().Single(r=>r.EntityID==Basis+2950).Shape=new MSB.Shape.Sphere{Radius=8};
        var center=map.Regions.GetEntries().Single(r=>r.EntityID==Basis+2950).Position;
        if(map.Regions.GetEntries().Any(r=>r.EntityID==CountdownSfx))throw new Exception("Countdown SFX region collision");
        map.Regions.Add(new MSBE.Region.SFX{Name="tower_countdown_ground",EntityID=CountdownSfx,EffectID=AuraFfxAssets.Gold,StartDisabled=1,Position=center-new Vector3(0,0.12f,0),Shape=new MSB.Shape.Point()});
        var visual=Local(Basis+7950);
        visual.Instructions.Add(new(2006,1,new object[]{CountdownSfx,(byte)0}));
        visual.Instructions.Add(new(1000,4,new object[]{(byte)0}));
        var cross=Local(Basis+7951);Flag(cross,Crossed,false);Wait(cross,Active,true);Wait(cross,45007993,true);
        cross.Instructions.Add(new(3,2,new object[]{(sbyte)0,(byte)1,10000u,Basis+2950,1}));
        Flag(cross,Crossed,true);Wait(cross,Active,false);cross.Instructions.Add(new(1000,4,new object[]{(byte)1}));
        foreach(var e in new[]{visual,cross}){events.Events[events.Events.FindIndex(old=>old.ID==e.ID)]=e;PersonalEvents.Verify(e);}
        var attendant=Local(Basis+7954);
        attendant.Instructions.Add(new(2004,5,new object[]{Basis+1810,(byte)0}));
        attendant.Instructions.Add(new(1000,4,new object[]{(byte)0}));
        events.Events.Add(attendant);
        events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new(2000,0,new object[]{0,(uint)attendant.ID,0u}));
    }
    public static void Verify(MSBE map,EMEVD events,Func<Vector3,Vector3> place) {
        var center=place(Vector3.Zero);
        foreach(uint id in new[]{Basis+2600,Basis+2951,Basis+2952}) {
            var region=map.Regions.GetEntries().Single(r=>r.EntityID==id);
            float yaw=region.Rotation.Y*MathF.PI/180;
            var toward=center-region.Position;toward.Y=0;
            if(Vector3.Dot(new(-MathF.Sin(yaw),0,-MathF.Cos(yaw)),Vector3.Normalize(toward))<0.99f)throw new Exception("Dragonlord arrival must face arena center");
        }
        var entry=map.Regions.GetEntries().Single(r=>r.EntityID==Basis+2600);
        if(map.Parts.Players.Any(p=>Vector3.Distance(p.Rotation,entry.Rotation)>.01f))throw new Exception("Dragonlord player facing differs from arrival");
        foreach(uint id in new[]{Basis+100,Basis+1810,Basis+1900}) {
            var part=map.Parts.GetEntries().Single(p=>p.EntityID==id);
            if(Vector3.Distance(part.Position,WorldPosition(id,place))>.01f)throw new Exception("Dragonlord service position mismatch");
        }
        if(map.Parts.Assets.Any(a=>a.EntityID==Gate))throw new Exception("Dragonlord entrance asset must be removed");
        var sfx=map.Regions.GetEntries().OfType<MSBE.Region.SFX>().Single(r=>r.EntityID==CountdownSfx);
        if(sfx.EffectID!=AuraFfxAssets.Gold||sfx.StartDisabled!=1||Vector3.Distance(sfx.Position,place(new Vector3(0,0.03f,0)))>.01f)throw new Exception("Countdown ground SFX configuration mismatch");
        var visual=events.Events.Single(e=>e.ID==Basis+7950);
        if(visual.Instructions.Any(i=>i.Bank==2006&&i.ID==2)||visual.Instructions.Count(i=>i.Bank==2006&&i.ID==1)!=1)throw new Exception("Legacy map countdown SFX must stay disabled");
        if(map.Regions.GetEntries().Single(r=>r.EntityID==Basis+2950).Shape is not MSB.Shape.Sphere {Radius:8})throw new Exception("Dragonlord center trigger must have radius 8");
        var cross=events.Events.Single(e=>e.ID==Basis+7951);
        if(cross.Instructions.Any(i=>(i.Bank==2004&&(i.ID==14||i.ID==42))||(i.Bank==3&&i.ID==24))||!cross.Instructions.Any(i=>i.Bank==3&&i.ID==2&&BitConverter.ToUInt32(i.ArgData,8)==Basis+2950))throw new Exception("Dragonlord must start on center entry without interaction or warp");
        foreach(uint id in new[]{Basis+7950,Basis+7951,Basis+7954})PersonalEvents.Verify(events.Events.Single(e=>e.ID==id));
    }
}
