using SoulsFormats;
using System.Globalization;
using System.Numerics;

// 主题战斗场地：复制进场设施，并按幕加入经过校验的联机敌人预置、站位和激活事件。
// 场地表：键|地图|锚点|名称|DLC|朝向偏移(度)|dx|dy|dz|幕。
//   地图写 battlefield 时用 v89 已发布的战场（battlefields.txt 的序号写在锚点栏），这里跳过；地图写 auto 时在全部地图里找锚点；锚点是原版敌人部件名（如 c4690_9000），或 model:5210（找第一个该模型的敌人），? 表示还没定。
// 做法：以锚点（原版 Boss 的出生点）为原点、它的朝向为前方，把宁姆格福竞技场（m45_02）里爬塔用的进场设施
// （召唤石、场边引导者、传送门、雾门、入口与雾门两侧的落地点）按竞技场里的相对位置搬过去，
// 编号保持不变（两张地图不会同时加载，原生 DLL 的入口与召唤石编号也就不用改）；事件文件换成竞技场里与这些设施有关的事件
// （先去掉模板联机事件，再生成目标幕的战斗预置事件），并关闭这张地图原有的全部敌人与 Boss 雾墙。
// 输出：map/mapstudio 与 event 下的地图与事件、arenas-built.txt（原生 DLL 按它选场地）。
// arena-candidates：列出地图里的 Boss 级敌人部件（名称、模型、NPC、位置、朝向），用来补全锚点。
static class ArenaAssets {
    const uint First=45020000,Last=45030000;
    // 联机预置敌人、守卫与它们的事件只在竞技场里用（47xxxxxx 是 Boss 血条的标记，要保留）
    static readonly (uint,uint)[] PartyRanges=[(48000000,48100000),(51000000,51100000)];
    // 已被其他功能改过、不能整张重写的地图
    static readonly string[] Protected=["m10_00_00_00","m11_00_00_00","m11_10_00_00","m12_01_00_00","m35_00_00_00","m30_09_00_00","m45_00_00_00","m45_01_00_00","m45_02_00_00"];
    static readonly string[] FogModels=["AEG099_001","AEG099_002","AEG099_003","AEG099_090","AEG099_230","AEG099_231","AEG099_232"];
    public record Site(string Key,string Map,string Anchor,string Name,bool Dlc,float Yaw,Vector3 Offset,int Act);
    static float F(string s)=>float.Parse(s,CultureInfo.InvariantCulture);
    public static Site[] Sites(string root)=>File.ReadAllLines(Path.Combine(root,"arena-sites.txt")).Where(l=>l.Trim().Length>0&&!l.StartsWith('#')).Select(l=>{
        var p=l.Split('|');
        return new Site(p[0],p[1],p[2],p[3],p[4]=="1",p.Length>5?F(p[5]):0,p.Length>8?new Vector3(F(p[6]),F(p[7]),F(p[8])):Vector3.Zero,p.Length>9?int.Parse(p[9]):0);
    }).ToArray();

    sealed class Archives:IDisposable {
        readonly List<(Dictionary<ulong,BHD5.FileHeader> files,FileStream data)> parts=new();
        public readonly string[] Maps;
        public Archives(string game) {
            foreach(var archive in new[]{"DLC","Data0","Data1","Data2","Data3"}) {
                var bhd=Path.Combine(game,archive+".bhd");if(!File.Exists(bhd))continue;
                using var header=UXM.CryptographyUtility.DecryptRsa(bhd,UXM.ArchiveKeys.EldenRingKeys[archive]);
                parts.Add((BHD5.Read(header,BHD5.Game.EldenRing).Buckets.SelectMany(b=>b).ToDictionary(f=>f.FileNameHash),File.OpenRead(Path.Combine(game,archive+".bdt"))));
            }
            Maps=File.ReadLines("work/UXM/UXM/res/EldenRingDictionary.txt").Where(p=>p.StartsWith("/map/mapstudio/")&&p.EndsWith(".msb.dcx")&&!p.EndsWith("_99.msb.dcx"))
                .Select(p=>p["/map/mapstudio/".Length..^".msb.dcx".Length]).Distinct().OrderBy(m=>m.StartsWith("m2")?0:1).ThenBy(m=>m).ToArray();
        }
        public byte[]? Read(string path) {
            ulong hash=0;foreach(var ch in path)hash=unchecked(hash*0x85+ch);
            foreach(var (files,data) in parts)if(files.TryGetValue(hash,out var file))return file.ReadFile(data);
            return null;
        }
        public MSBE? Map(string map)=>Read($"/map/mapstudio/{map}.msb.dcx") is {} b?MSBE.Read(b):null;
        public void Dispose(){foreach(var (_,data) in parts)data.Dispose();}
    }
    static MSBE.Part.Enemy? FindAnchor(MSBE map,string anchor) {
        if(anchor.StartsWith("model:"))return map.Parts.Enemies.FirstOrDefault(e=>e.ModelName==$"c{int.Parse(anchor[6..]):D4}");
        return map.Parts.Enemies.FirstOrDefault(e=>e.Name==anchor);
    }
    /// 竞技场坐标（原点为竞技场中心、朝向 0）→ 场地坐标：与原生 Arena::position 相同的旋转
    static Vector3 Place(Vector3 local,Vector3 center,float yawDeg) {
        float yaw=yawDeg*MathF.PI/180,sin=MathF.Sin(yaw),cos=MathF.Cos(yaw);
        return new(center.X+local.X*cos+local.Z*sin,center.Y+local.Y,center.Z-local.X*sin+local.Z*cos);
    }
    static bool Ours(uint id)=>id>=First&&id<Last;
    // 要搬的进场设施：召唤石、场边引导者；传送门、雾门；入口、门内落地点、门外落地点与朝向点
    static readonly uint[] Enemies=[First+100,First+1810],Assets=[First+1900,First+1950],Regions=[First+2101,First+2600,First+2950,First+2951,First+2952];
    static bool PartyArg(EMEVD.Instruction i) {
        for(int o=0;o+4<=i.ArgData.Length;o+=4){uint v=BitConverter.ToUInt32(i.ArgData,o);if(PartyRanges.Any(r=>v>=r.Item1&&v<r.Item2))return true;}
        return false;
    }

    public static void Candidates(string game,string root) {
        using var archives=new Archives(game);
        var wanted=Sites(root).Where(s=>s.Map!="battlefield"&&(s.Anchor=="?"||s.Map=="auto")).Select(s=>s.Key).ToArray();
        var models=File.ReadLines(Path.Combine(root,"boss-pool.txt")).Where(l=>!string.IsNullOrWhiteSpace(l)&&!l.StartsWith('#')).Select(l=>"c"+int.Parse(l.Split('|')[3]).ToString("D4")).ToHashSet();
        Console.WriteLine($"Sites still needing anchors: {string.Join(",",wanted)}");
        foreach(var map in archives.Maps.Where(m=>!m.StartsWith("m60")&&!m.StartsWith("m61"))) {
            var msb=archives.Map(map);if(msb==null)continue;
            // Boss 级部件：原版 Boss 大多在 9000 号以后（cXXXX_9000），按名字粗筛
            foreach(var e in msb.Parts.Enemies.Where(e=>(e.Name.Contains("_9")||models.Contains(e.ModelName))&&e.NPCParamID>0))
                Console.WriteLine(string.Join('|',map,e.Name,e.ModelName,e.NPCParamID,e.Position.X.ToString("0.00",CultureInfo.InvariantCulture),e.Position.Y.ToString("0.00",CultureInfo.InvariantCulture),e.Position.Z.ToString("0.00",CultureInfo.InvariantCulture),e.Rotation.Y.ToString("0.0",CultureInfo.InvariantCulture)));
        }
    }

    public static void Build(string game,string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var template=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_02_00_00.msb.dcx"));
        var templateEvents=EMEVD.Read(Path.Combine(input,"event/m45_02_00_00.emevd.dcx"));
        using var archives=new Archives(game);
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
        var built=new List<string>{"# 键|地图|名称|中心x|y|z|朝向|引导者x|y|z|幕|DLC|联机预置"};
        // 竞技场里爬塔的事件：去掉联机预置敌人与守卫的事件；事件 0 只保留初始化这些事件和标记的指令
        var keptEvents=templateEvents.Events.Where(e=>e.ID!=0&&!e.Instructions.Any(PartyArg)).ToList();
        var keptIds=keptEvents.Select(e=>e.ID).ToHashSet();
        var templateInit=templateEvents.Events.Single(e=>e.ID==0);
        var initKept=templateInit.Instructions.Where(i=>!PartyArg(i)&&(i.Bank!=2000||i.ID!=0||keptIds.Contains(BitConverter.ToUInt32(i.ArgData,4)))).ToList();
        var center0=Vector3.Zero;
        var generated=new HashSet<string>();
        foreach(var site in Sites(input)) {
            if(site.Map=="battlefield")continue;
            if(site.Act<1||site.Act>3)throw new Exception(site.Key+": missing act assignment");
            if(site.Anchor=="?"){Console.WriteLine($"Skip {site.Key}: anchor not chosen yet (run arena-candidates)");continue;}
            string map=site.Map;MSBE? msb=null;MSBE.Part.Enemy? anchor=null;
            if(map=="auto") {
                foreach(var m in archives.Maps.Where(m=>!m.StartsWith("m60")&&!m.StartsWith("m61"))) {var t=archives.Map(m);var a=t==null?null:FindAnchor(t,site.Anchor);if(a!=null){map=m;msb=t;anchor=a;break;}}
            } else {msb=archives.Map(map);anchor=msb==null?null:FindAnchor(msb,site.Anchor);}
            if(msb==null||anchor==null){Console.WriteLine($"Skip {site.Key}: {(msb==null?"map not found (DLC not installed?)":"anchor "+site.Anchor+" not found")} in {map}");continue;}
            if(Protected.Contains(map))throw new Exception($"{site.Key}: {map} is used by another feature and cannot be rewritten");
            if(!generated.Add(map))throw new Exception($"Multiple arena sites would overwrite {map}");
            var events=EMEVD.Read(archives.Read($"/event/{map}.emevd.dcx")??throw new Exception($"{site.Key}: event file missing"));
            float yaw=anchor.Rotation.Y+site.Yaw;var center=anchor.Position+site.Offset;
            Vector3 Service(Vector3 p)=>Place(new Vector3(p.X*0.45f,p.Y,p.Z*0.45f),center,yaw);
            bool dragonlord=site.Key=="farum_dragonlord";
            Vector3 ServiceAt(uint id,Vector3 p)=>dragonlord?DragonlordEntrance.WorldPosition(id,v=>Place(v,center,yaw)):Service(p);
            if(msb.Parts.GetEntries().Any(p=>Ours(p.EntityID))||msb.Regions.GetEntries().Any(r=>Ours(r.EntityID)))throw new Exception($"{site.Key}: {map} already uses tower entity ids");
            // 原版敌人全部关闭（没有编号的补上编号），Boss 雾墙关闭
            var init=new EMEVD.Event(0);
            uint assigned=49000000;var used=msb.Parts.GetEntries().Select(p=>p.EntityID).ToHashSet();
            uint Assign(){while(used.Contains(assigned))assigned++;used.Add(assigned);return assigned++;}
            foreach(var enemy in msb.Parts.Enemies) {
                if(enemy.EntityID==0||enemy.EntityID==uint.MaxValue)enemy.EntityID=Assign();
                init.Instructions.Add(new EMEVD.Instruction(2004,5,new object[]{enemy.EntityID,(byte)0}));
            }
            foreach(var fog in msb.Parts.Assets.Where(a=>FogModels.Contains(a.ModelName))) {
                if(fog.EntityID==0||fog.EntityID==uint.MaxValue)fog.EntityID=Assign();
                init.Instructions.Add(new EMEVD.Instruction(2005,3,new object[]{fog.EntityID,(byte)0}));
            }
            // 搬进场设施：位置按锚点变换，地面碰撞统一挂在锚点所在的碰撞上
            foreach(var source in template.Parts.Enemies.Where(e=>Enemies.Contains(e.EntityID))) {
                var part=(MSBE.Part.Enemy)source.DeepCopy();part.Name="tower_site_"+part.EntityID;
                part.Position=ServiceAt(source.EntityID,source.Position-center0);part.Rotation=new(source.Rotation.X,dragonlord?yaw-90:source.Rotation.Y+yaw,source.Rotation.Z);
                part.CollisionPartName=anchor.CollisionPartName;part.EntityGroupIDs=new uint[8];
                PartyPlacement.Prepare(part);
                if(!msb.Models.Enemies.Any(m=>m.Name==part.ModelName))msb.Models.Enemies.Add((MSBE.Model.Enemy)template.Models.Enemies.Single(m=>m.Name==part.ModelName).DeepCopy());
                msb.Parts.Enemies.Add(part);
            }
            foreach(var source in template.Parts.Assets.Where(a=>Assets.Contains(a.EntityID))) {
                var part=(MSBE.Part.Asset)source.DeepCopy();part.Name="tower_site_"+part.EntityID;
                // 传送门放到场地正中（原版 Boss 的出生点）：战斗中关闭，胜利后从地面钻出来（PortalReveal）
                part.Position=source.EntityID==First+1900?Place(new Vector3(0,source.Position.Y-center0.Y,0),center,yaw):ServiceAt(source.EntityID,source.Position-center0);
                part.Rotation=new(source.Rotation.X,source.Rotation.Y+yaw,source.Rotation.Z);
                part.InstanceID=msb.Parts.Assets.Where(a=>a.ModelName==part.ModelName).Select(a=>a.InstanceID).DefaultIfEmpty(-1).Max()+1;
                part.EntityGroupIDs=new uint[8];part.UnkPartNames=new string[6];part.UnkT54PartName=null;part.MapStudioLayer=uint.MaxValue;
                part.Unk1.DisplayGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();part.Unk1.DrawGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();
                if(!msb.Models.Assets.Any(m=>m.Name==part.ModelName))msb.Models.Assets.Add((MSBE.Model.Asset)template.Models.Assets.Single(m=>m.Name==part.ModelName).DeepCopy());
                msb.Parts.Assets.Add(part);
            }
            foreach(var source in template.Regions.GetEntries().Where(r=>Regions.Contains(r.EntityID))) {
                var region=source.DeepCopy();region.Name="tower_site_"+region.EntityID;region.ActivationPartName=null;
                bool summon=source.EntityID==First+2101;
                region.Position=summon?Place(source.Position,center,yaw):ServiceAt(source.EntityID,source.Position-center0);
                region.Rotation=new(source.Rotation.X,dragonlord&&!summon?yaw+90:source.Rotation.Y+yaw,source.Rotation.Z);
                msb.Regions.Add(region);
            }
            // 默认出生点放到入口
            var entry=msb.Regions.GetEntries().Single(r=>r.EntityID==First+2600);
            foreach(var player in msb.Parts.Players){player.Position=entry.Position;player.Rotation=entry.Rotation;}
            PartyModels.Prepare(msb);PartyModels.Verify(msb);
            // 事件：竞技场的爬塔事件 + 关闭原版敌人与雾墙；原地图事件不再运行
            events.Events=keptEvents.Select(e=>EMEVD.Read(new EMEVD{Events=[e]}.Write()).Events[0]).ToList();
            foreach(var i in initKept)init.Instructions.Add(i);
            events.Events.Insert(0,init);
            PortalReveal.AddRegion(msb,First+1900);PortalReveal.Apply(events,First+1900);
            if(dragonlord)DragonlordEntrance.Apply(msb,events);
            int[] partyActs=site.Key=="farum_dragonlord"?[1,2,3]:[site.Act];
            foreach(int act in partyActs)ArenaPartyAssets.Add(input,msb,events,act,anchor,p=>Place(p,center,yaw),yaw);
            DormantLoadingAssets.Apply(msb,events);
            if(msb.Parts.Enemies.Select(e=>e.Name).Distinct().Count()!=msb.Parts.Enemies.Count)throw new Exception("Duplicate arena enemy names");
            msb.Write(Path.Combine(output,"map/mapstudio",map+".msb.dcx"));events.Write(Path.Combine(output,"event",map+".emevd.dcx"));
            Verify(MSBE.Read(Path.Combine(output,"map/mapstudio",map+".msb.dcx")),EMEVD.Read(Path.Combine(output,"event",map+".emevd.dcx")),template,site.Key);
            foreach(int act in partyActs)ArenaPartyAssets.Verify(MSBE.Read(Path.Combine(output,"map/mapstudio",map+".msb.dcx")),EMEVD.Read(Path.Combine(output,"event",map+".emevd.dcx")),MSBE.Read(Path.Combine(input,"map/mapstudio",$"m45_0{3-act}_00_00.msb.dcx")),act);
            if(dragonlord)DragonlordEntrance.Verify(MSBE.Read(Path.Combine(output,"map/mapstudio",map+".msb.dcx")),EMEVD.Read(Path.Combine(output,"event",map+".emevd.dcx")),p=>Place(p,center,yaw));
            var attendant=ServiceAt(First+1810,template.Parts.Enemies.Single(e=>e.EntityID==First+1810).Position-center0);
            string N(float v)=>v.ToString("0.###",CultureInfo.InvariantCulture);
            built.Add(string.Join('|',site.Key,map,site.Name,N(center.X),N(center.Y),N(center.Z),N(yaw),N(attendant.X),N(attendant.Y),N(attendant.Z),site.Act,site.Dlc?1:0,1));
            Console.WriteLine($"Built arena {site.Key}: {map} anchor {anchor.Name} at ({N(center.X)},{N(center.Y)},{N(center.Z)}) yaw {N(yaw)}");
        }
        File.WriteAllLines(Path.Combine(output,"arenas-built.txt"),built);
        var resources=new List<string>();
        foreach(var map in generated)foreach(var relative in new[]{"map/mapstudio/"+map+".msb.dcx","event/"+map+".emevd.dcx"}) {
            var bytes=File.ReadAllBytes(Path.Combine(output,relative));ulong hash=14695981039346656037;
            foreach(byte b in bytes)hash=unchecked((hash^b)*1099511628211);
            resources.Add($"{relative}|{bytes.Length}|{hash:x16}");
        }
        File.WriteAllLines(Path.Combine(output,"arenas-resources.txt"),resources.OrderBy(r=>r,StringComparer.Ordinal));
        Console.WriteLine($"{built.Count-1} arenas built; copy map/, event/ and arenas-built.txt into scene-assets");
    }
    static void Verify(MSBE map,EMEVD events,MSBE template,string key) {
        foreach(var id in Enemies)if(map.Parts.Enemies.Count(e=>e.EntityID==id)!=1)throw new Exception($"{key}: missing tower part {id}");
        foreach(var id in Assets.Where(id=>key!="farum_dragonlord"||id!=First+1950))if(map.Parts.Assets.Count(a=>a.EntityID==id)!=1)throw new Exception($"{key}: missing tower asset {id}");
        foreach(var id in Regions)if(map.Regions.GetEntries().Count(r=>r.EntityID==id)!=1)throw new Exception($"{key}: missing tower region {id}");
        var summon=map.Regions.GetEntries().Single(r=>r.EntityID==First+2101);
        var original=template.Regions.GetEntries().Single(r=>r.EntityID==First+2101);
        if(summon.Shape is not MSB.Shape.Sphere sphere||original.Shape is not MSB.Shape.Sphere expected||sphere.Radius!=expected.Radius)throw new Exception($"{key}: summon range changed or missing");
        foreach(uint id in new[]{First+2600,First+2950,First+2951}) {
            var position=map.Regions.GetEntries().Single(r=>r.EntityID==id).Position;
            if(Vector3.Distance(position,summon.Position)>=sphere.Radius)throw new Exception($"{key}: player arrival outside summon region");
        }
        if(map.Parts.Enemies.Single(e=>e.EntityID==First+100).TalkID!=template.Parts.Enemies.Single(e=>e.EntityID==First+100).TalkID)throw new Exception($"{key}: summon stone talk changed");
        foreach(var part in map.Parts.Enemies.Where(e=>Ours(e.EntityID)))if(!map.Parts.Collisions.Any(c=>c.Name==part.CollisionPartName))throw new Exception($"{key}: {part.Name} has no ground collision");
        PortalReveal.Verify(map,events,First+1900,key);
        var init=events.Events.Single(e=>e.ID==0);
        foreach(var enemy in map.Parts.Enemies.Where(e=>!Ours(e.EntityID)&&!ArenaPartyAssets.Combat(e.EntityID)))
            if(!init.Instructions.Any(i=>i.Bank==2004&&i.ID==5&&BitConverter.ToUInt32(i.ArgData,0)==enemy.EntityID&&i.ArgData[4]==0))throw new Exception($"{key}: vanilla enemy {enemy.Name} left active");
    }
}
