using SoulsFormats;

static class MelinaAssets {
    public const int PreviewFirst=9510000,PreviewMenu=33;
    // 与 native/src/enemies.rs 的 PREVIEW_NAMES 相同：沿用追加顺序，已有名称编号不变。
    public static string[] PreviewNames(string dir) {
        var pool=File.ReadAllLines(Path.Combine(dir,"enemy-pools.txt")).Select(l=>l.Split('|')).ToArray();
        bool Solo(string[] p)=>p.Length>5&&p[5]=="dlc";
        // 新 Boss、新普通怪依次放在 DLC 小怪之后，不因多人支持改变排序。
        var bosses=File.ReadAllLines(Path.Combine(dir,"boss-pool.txt")).Select(l=>l.Split('|')).ToArray();
        bool SoloBoss(string[] p)=>p.Length>10&&p[10].Length>0;
        bool SoloEnemy(string[] p)=>p.Length>5&&(p[5]=="solo"||p[5]=="party");
        // 序号 153 起的 Boss（DLC 的融泥骑士、黄金河马、贝勒、拉塔恩、梅瑟莫）排在新普通怪之后，与 native/src/enemies.rs 的 LATE_BOSS_FIRST 相同
        bool Late(string[] p)=>int.Parse(p[2])>=153;
        return pool.Where(p=>!Solo(p)&&!SoloEnemy(p)).Select(p=>p[4])
            .Concat(bosses.Where(p=>!SoloBoss(p)).Select(p=>p[7]))
            .Concat(pool.Where(Solo).Select(p=>p[4]))
            .Concat(bosses.Where(p=>SoloBoss(p)&&!Late(p)).Select(p=>p[7]))
            .Concat(pool.Where(SoloEnemy).Select(p=>p[4]))
            .Concat(bosses.Where(Late).Select(p=>p[7])).Distinct().ToArray();
    }
    public static IEnumerable<(int id,string text)> PreviewTexts(string[] names) {
        for(int i=0;i<names.Length;i++)for(int n=1;n<=15;n++)yield return (PreviewFirst+i*16+n,$"{names[i]} ×{n}");
    }
    public static void Patch(string input,string output) {
        var template=MSBE.Read("work/boss-report/maps/m11_00_00_00.msb.dcx").Parts.Enemies.Single(e=>e.ModelName=="c2180");
        var rows=File.ReadAllLines("outputs/erdtree-tower/scene-assets/party-services.txt").Select(l=>l.Split('|')).Where(p=>p[7]=="5").Select(p=>(Map:p[0],Id:uint.Parse(p[2])))
            .Append(("m45_00_00_00",45001802u));
        foreach(var group in rows.GroupBy(p=>p.Item1)) {
            string name="map/mapstudio/"+group.Key+".msb.dcx";
            var map=MSBE.Read(Path.Combine(input,name));
            foreach(var row in group) {
                int index=map.Parts.Enemies.FindIndex(e=>e.EntityID==row.Item2);
                if(index<0)throw new Exception("Service entity missing");
                var old=map.Parts.Enemies[index];var part=(MSBE.Part.Enemy)template.DeepCopy();
                part.Name=old.Name;part.EntityID=old.EntityID;part.InstanceID=old.InstanceID;
                part.Position=old.Position;part.Rotation=old.Rotation;part.CollisionPartName=old.CollisionPartName;
                part.EntityGroupIDs=new uint[8];part.TalkID=-1;part.WalkRouteName=null;part.BackupEventAnimID=-1;part.ChrActivateCondParamID=0;
                PartyPlacement.Prepare(part);map.Parts.Enemies[index]=part;
            }
            if(!map.Models.Enemies.Any(m=>m.Name=="c2180"))map.Models.Enemies.Add(new MSBE.Model.Enemy{Name="c2180"});
            PartyModels.Prepare(map);PartyModels.Verify(map);
            var path=Path.Combine(output,name);Directory.CreateDirectory(Path.GetDirectoryName(path)!);map.Write(path);
            var check=MSBE.Read(path);
            foreach(var row in group) {
                var e=check.Parts.Enemies.Single(e=>e.EntityID==row.Item2);var expected=map.Parts.Enemies.Single(p=>p.EntityID==row.Item2);
                if(e.ModelName!="c2180"||e.NPCParamID!=21801034||e.ThinkParamID!=21800000||e.Position!=expected.Position||e.Rotation!=expected.Rotation||e.CollisionPartName!=expected.CollisionPartName||e.TalkID!=-1)throw new Exception("Melina service roundtrip mismatch");
                PartyPlacement.Verify(e);
            }
            Console.WriteLine($"Verified Melina services: {group.Key}, {group.Count()}");
        }
        const string msg="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,msg));var file=menu.Files.Single(f=>f.ID==33);var fmg=FMG.Read(file.Bytes);
        var texts=new Dictionary<int,string>{{9500100,"圣杯瓶状态"},{9500101,"雾门内的敌人"},{9500102,"当前没有待挑战的敌人"},{9500104,"通过种子开始游戏"},{9500105,"只有房主可以用种子开局，房主设定的种子会同步给全队。"}};
        for(int n=0;n<=255;n++){texts[9501000+n]=$"红圣杯瓶上限：{n}";texts[9501300+n]=$"蓝圣杯瓶上限：{n}";}
        var names=PreviewNames("outputs/erdtree-tower/scene-assets");
        foreach(var (id,text) in PreviewTexts(names))texts[id]=text;
        foreach(var (id,text) in texts){fmg.Entries.RemoveAll(e=>e.ID==id);fmg.Entries.Add(new(id,text));}
        file.Bytes=fmg.Write();var dest=Path.Combine(output,msg);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);menu.Write(dest);
        var verified=FMG.Read(BND4.Read(dest).Files.Single(f=>f.ID==33).Bytes);
        foreach(var (id,text) in texts)if(verified.Entries.Single(e=>e.ID==id).Text!=text)throw new Exception("Navigation text mismatch");
        Console.WriteLine($"Verified flask limits and {names.Length} enemy names");
    }
}
