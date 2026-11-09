using SoulsFormats;
using System.Numerics;

static class HiddenRoadAssets {
    public static void Build(string original,string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        foreach(var (name,entry,anchor,goal) in new[]{
            ("m60_52_57_00",1052570988u,"c3500_9004",new Vector3(68.088f,1683.763f,-48.374f))}) {
            var map=MSBE.Read(Path.Combine(original,name+".msb.dcx"));
            var source=map.Parts.GetEntries().Single(p=>p.Name==anchor);
            var start=source.Position+new Vector3(0,.2f,0);
            if(name=="m60_52_57_00") {
                using var config=System.Text.Json.JsonDocument.Parse(File.ReadAllText(Path.Combine(input,"heretical-rise-start.json")));
                var root=config.RootElement;var xyz=root.GetProperty("position").EnumerateArray().Select(x=>x.GetSingle()).ToArray();
                var clearance=root.GetProperty("clearance").GetSingle();
                if(root.GetProperty("map").GetString()!=name||xyz.Length!=3||xyz.Any(x=>!float.IsFinite(x))||clearance is <0 or >0.3f)throw new Exception("Invalid measured hidden-road start");
                start=new Vector3(xyz[0],xyz[1]+clearance,xyz[2]);
            }
            float yaw=MathF.Atan2(-(goal.X-start.X),-(goal.Z-start.Z))*180/MathF.PI;
            if(map.Parts.GetEntries().Any(p=>p.EntityID==entry))throw new Exception("Hidden road entry collision");
            var player=(MSBE.Part.Player)map.Parts.Players.First().DeepCopy();
            player.Name="tower_hidden_road_start";player.EntityID=entry;map.Parts.Players.Add(player);
            foreach(var p in map.Parts.Players){p.Position=start;p.Rotation=new(0,yaw,0);}
            map.Events.Treasures.Clear();
            var events=EMEVD.Read(Path.Combine(original,name+".emevd.dcx"));
            events.Events.Clear();var init=new EMEVD.Event(0);PersonalEvents.Local(init);
            uint anonymous=entry+4000;
            foreach(var enemy in map.Parts.Enemies) {
                if(enemy.EntityID==0||enemy.EntityID==uint.MaxValue){while(map.Parts.GetEntries().Any(p=>p.EntityID==anonymous))anonymous++;enemy.EntityID=anonymous++;}
                init.Instructions.Add(new(2004,5,new object[]{enemy.EntityID,(byte)0}));
                init.Instructions.Add(new(2004,29,new object[]{enemy.EntityID,(byte)1}));
            }
            events.Events.Add(init);
            var mapPath=Path.Combine(output,"map/mapstudio",name+".msb.dcx");
            var eventPath=Path.Combine(output,"event",name+".emevd.dcx");
            Directory.CreateDirectory(Path.GetDirectoryName(mapPath)!);Directory.CreateDirectory(Path.GetDirectoryName(eventPath)!);
            map.Write(mapPath);events.Write(eventPath);
            var check=MSBE.Read(mapPath);var ec=EMEVD.Read(eventPath);
            if(check.Parts.Players.Single(p=>p.EntityID==entry).Position!=start||check.Events.Treasures.Count!=0)throw new Exception("Hidden road spawn/reward verification failed");
            var native=MSBE.Read(Path.Combine(original,name+".msb.dcx"));
            if(check.Parts.Collisions.Count!=native.Parts.Collisions.Count||check.Parts.Assets.Count!=native.Parts.Assets.Count||ec.Events.Count!=1||ec.Events[0].Instructions.Count!=init.Instructions.Count)throw new Exception("Hidden road geometry/event verification failed");
            PersonalEvents.Verify(ec.Events[0]);
            Console.WriteLine($"Verified {name}: native anchor {anchor} start={start}, goal={goal}, {check.Parts.Collisions.Count} collision parts preserved; live route test pending");
            check.Parts.Players.Clear();check.Parts.Players.AddRange(native.Parts.Players);
            check.Parts.Enemies.Clear();check.Parts.Enemies.AddRange(native.Parts.Enemies);
            check.Events.Treasures.Clear();check.Events.Treasures.AddRange(native.Events.Treasures);
            if(!check.Write().SequenceEqual(native.Write()))throw new Exception("Unrelated hidden-road geometry changed");
        }
        var path=Path.Combine(input,"msg/zhocn/menu_dlc02.msgbnd.dcx");var text=BND4.Read(path);
        var file=text.Files.Single(f=>f.ID==33);var fmg=FMG.Read(file.Bytes);
        fmg.Entries.RemoveAll(e=>e.ID is 9500211 or 9500213);
        (int,string)[] entries=[(9500210,"异端魔法师塔·隐形桥"),(9500212,"异端魔法师塔·隐形桥"),
            (9500214,"穿过异端魔法师塔的隐形桥与空中阶梯，抵达塔楼入口，获得两个随机护符赐福。\n联机提交路线投票，存活队员全部到达后结算。\n确定参加？")];
        foreach(var (id,value) in entries){fmg.Entries.RemoveAll(e=>e.ID==id);fmg.Entries.Add(new(id,value));}
        file.Bytes=fmg.Write();var target=Path.Combine(output,"msg/zhocn/menu_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(target)!);text.Write(target);
        var restored=FMG.Read(BND4.Read(target).Files.Single(f=>f.ID==33).Bytes);
        if(entries.Any(e=>restored.Entries.Single(x=>x.ID==e.Item1).Text!=e.Item2))throw new Exception("Hidden road menu text mismatch");
        var prior=FMG.Read(BND4.Read(path).Files.Single(f=>f.ID==33).Bytes);
        foreach(var e in prior.Entries.Where(e=>e.ID is not (9500211 or 9500213)&&!entries.Any(x=>x.Item1==e.ID)))if(restored.Entries.Single(x=>x.ID==e.ID).Text!=e.Text)throw new Exception("Unrelated menu text changed");
        var files=Directory.GetFiles(Path.Combine(output,"map/mapstudio")).Concat(Directory.GetFiles(Path.Combine(output,"event"))).Order().ToArray();
        File.WriteAllLines(Path.Combine(output,"hidden-road-resources.txt"),files.Select(p=>{
            var bytes=File.ReadAllBytes(p);ulong h=0xcbf29ce484222325;foreach(byte b in bytes)h=unchecked((h^b)*0x100000001b3);
            return $"{Path.GetRelativePath(output,p).Replace('\\','/')}|{bytes.Length}|{h:x16}";
        }));
    }
}
