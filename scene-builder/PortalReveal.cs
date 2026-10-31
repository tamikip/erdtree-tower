using SoulsFormats;
using System.Numerics;

// 战斗场地的传送门：放在场地正中，战斗中关闭（看不见、不能交互），胜利后从地面“钻出来”。
// 做法：在传送门脚下加一个默认关闭的地面金光特效区域（AuraFfxAssets.Gold，与龙王场地倒计时用的是同一个持续地面特效）；
// 改传送门的光效事件（给传送门挂 2006[04] 光效的那个事件）——事件开头（关闭联机同步之后）关闭传送门与地面金光，
// 原来挂光效的位置之前改成：地面金光亮起 → 等 RiseDelay 秒 → 传送门出现 → 再挂原来的传送门光效。
// 光效事件在胜利旗标熄灭后会重启，又回到关闭状态。营地 / 商店 / 宝箱房的传送门不受影响（只在主题竞技场与独立战场调用）。
static class PortalReveal {
    /// 地面金光亮起后过多久传送门出现（秒）
    public const float RiseDelay=0.6f;
    /// 地面金光特效区域的编号：传送门编号 + 1062（basis+2962；龙王场地的倒计时特效占用 basis+2960）
    public static uint RiseRegion(uint portal)=>portal+1062;

    /// 在传送门脚下加地面金光区域（默认关闭）
    public static void AddRegion(MSBE map,uint portal) {
        var part=map.Parts.Assets.Single(a=>a.EntityID==portal);
        uint id=RiseRegion(portal);
        if(map.Regions.GetEntries().Any(r=>r.EntityID==id)||map.Parts.GetEntries().Any(p=>p.EntityID==id))throw new Exception($"Portal rise SFX id {id} already used");
        map.Regions.Add(new MSBE.Region.SFX{Name="tower_portal_rise",EntityID=id,EffectID=AuraFfxAssets.Gold,StartDisabled=1,Position=part.Position-new Vector3(0,0.12f,0),Shape=new MSB.Shape.Point()});
    }

    /// 改传送门的光效事件，并在事件 0 里开场就关掉传送门
    public static void Apply(EMEVD events,uint portal) {
        bool Glow(EMEVD.Instruction i)=>i.Bank==2006&&i.ID==4&&BitConverter.ToUInt32(i.ArgData,0)==portal;
        var visual=events.Events.Single(e=>e.Instructions.Any(Glow));
        uint rise=RiseRegion(portal);
        int at=visual.Instructions.FindIndex(Glow);
        visual.Instructions.InsertRange(at,new[]{
            new EMEVD.Instruction(2006,2,new object[]{rise}),
            new EMEVD.Instruction(1001,0,new object[]{RiseDelay}),
            new EMEVD.Instruction(2005,3,new object[]{portal,(byte)1}),
        });
        // 本地事件的第一条是“关闭联机同步”，关闭指令插在它后面
        int start=visual.Instructions.FirstOrDefault() is {Bank:2000,ID:2}?1:0;
        visual.Instructions.InsertRange(start,new[]{
            new EMEVD.Instruction(2005,3,new object[]{portal,(byte)0}),
            new EMEVD.Instruction(2006,1,new object[]{rise,(byte)0}),
        });
        events.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2005,3,new object[]{portal,(byte)0}));
    }

    public static void Verify(MSBE map,EMEVD events,uint portal,string key) {
        uint rise=RiseRegion(portal);
        var region=map.Regions.GetEntries().OfType<MSBE.Region.SFX>().SingleOrDefault(r=>r.EntityID==rise)??throw new Exception($"{key}: missing portal rise SFX region");
        var part=map.Parts.Assets.Single(a=>a.EntityID==portal);
        if(region.EffectID!=AuraFfxAssets.Gold||region.StartDisabled!=1||Vector3.Distance(region.Position,part.Position-new Vector3(0,0.12f,0))>.01f)throw new Exception($"{key}: portal rise SFX misplaced");
        bool Is(EMEVD.Instruction i,int bank,int id,uint entity)=>i.Bank==bank&&i.ID==id&&BitConverter.ToUInt32(i.ArgData,0)==entity;
        var visual=events.Events.Single(e=>e.Instructions.Any(i=>Is(i,2006,4,portal)));
        var list=visual.Instructions;
        int glow=list.FindIndex(i=>Is(i,2006,4,portal));
        if(glow<3||!Is(list[glow-1],2005,3,portal)||list[glow-1].ArgData[4]!=1||list[glow-2].Bank!=1001||!Is(list[glow-3],2006,2,rise))throw new Exception($"{key}: portal reveal sequence missing");
        int off=list.FindIndex(i=>Is(i,2005,3,portal)&&i.ArgData[4]==0);
        if(off<0||off>glow||!list.Any(i=>Is(i,2006,1,rise)))throw new Exception($"{key}: portal must start hidden");
        if(!events.Events.Single(e=>e.ID==0).Instructions.Any(i=>Is(i,2005,3,portal)&&i.ArgData[4]==0))throw new Exception($"{key}: portal not hidden at map load");
        foreach(var i in list.Where(i=>i.Bank==2005||i.Bank==2006||i.Bank==1001)) {
            int expected=(i.Bank,i.ID) switch{(2005,3)=>8,(2006,1)=>8,(2006,2)=>4,(2006,4)=>12,(2006,5)=>8,(1001,0)=>4,_=>-1};
            if(expected>=0&&i.ArgData.Length!=expected)throw new Exception($"{key}: portal event instruction layout mismatch {i.Bank}[{i.ID}]");
        }
    }

    /// 自检：按竞技场传送门光效事件的结构造一份事件与地图，改完后写出再读回，检查顺序与指令格式
    public static void Test() {
        const uint basis=45020000,portal=basis+1900;
        var events=new EMEVD();var init=new EMEVD.Event(0);events.Events.Add(init);
        var visual=new EMEVD.Event(basis+986,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(visual);
        visual.Instructions.Add(new EMEVD.Instruction(2006,5,new object[]{portal,1}));
        visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)-1,(byte)1,(byte)0,45020970u}));
        visual.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)-1}));
        visual.Instructions.Add(new EMEVD.Instruction(2006,4,new object[]{portal,200,0xC4FD6}));
        visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,45020970u}));
        visual.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
        events.Events.Add(visual);
        var map=new MSBE();map.Parts.Assets.Add(new MSBE.Part.Asset{Name="tower_site_portal",EntityID=portal,Position=new Vector3(1,2,3)});
        AddRegion(map,portal);Apply(events,portal);
        var copy=EMEVD.Read(events.Write());
        Verify(map,copy,portal,"test");PersonalEvents.Verify(copy.Events.Single(e=>e.ID==basis+986));
        var order=copy.Events.Single(e=>e.ID==basis+986).Instructions.Select(i=>$"{i.Bank}[{i.ID}]").ToArray();
        Console.WriteLine("Portal reveal event: "+string.Join(" ",order));
        if(string.Join(" ",order)!="2000[2] 2005[3] 2006[1] 2006[5] 3[0] 0[0] 2006[2] 1001[0] 2005[3] 2006[4] 3[0] 1000[4]")throw new Exception("Portal reveal order changed");
        Console.WriteLine("Portal reveal self-test passed");
    }
}
