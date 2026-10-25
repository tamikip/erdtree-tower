using SoulsFormats;
using System.Numerics;
using System.Globalization;

static class PartyServiceAssets {
    public static void TestLoading() {
        const uint id=45021810;
        var ev=Visibility(id);
        int on=ev.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==5&&i.ArgData[4]==1);
        int load=ev.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==29&&i.ArgData[4]==0);
        int off=ev.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==29&&i.ArgData[4]==1);
        if(off<0||load<=off||load>=on)throw new Exception("Service must request model loading before enabling and remove it while inactive");
        if(ev.Instructions.Where(i=>i.Bank==2004&&i.ID==29).Any(i=>i.ArgData.Length!=8||BitConverter.ToUInt32(i.ArgData)!=id))throw new Exception("Service backread ABI mismatch");
        var file=new EMEVD();file.Events.Add(ev);PersonalEvents.Verify(EMEVD.Read(file.Write()).Events.Single());
        Console.WriteLine("Verified local service loading/unloading order and serialized ABI");
    }
    public static EMEVD.Event Visibility(uint id) {
        var ev=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);
        ev.Instructions.Add(new(2004,5,new object[]{id,(byte)0}));
        ev.Instructions.Add(new(2004,29,new object[]{id,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,id+70}));
        ev.Instructions.Add(new(2004,35,new object[]{id,(byte)1}));
        ev.Instructions.Add(new(2004,29,new object[]{id,(byte)0}));
        ev.Instructions.Add(new(2004,5,new object[]{id,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,id+70}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));return ev;
    }
    public static void Build(string input,string output) {
        var rows=File.ReadAllLines("outputs/erdtree-tower/scene-assets/party-services.txt").Select(l=>l.Split('|')).ToArray();
        var hub=MSBE.Read("work/scene-cache/m11_10_00_00.msb.dcx");
        var templates=hub.Parts.Enemies.Where(e=>e.EntityID is 11100730 or 11100705).ToList();
        foreach(var path in Directory.EnumerateFiles("work/boss-report/maps","*.msb.dcx")) {
            if(templates.Any(e=>e.ModelName=="c3200"))break;
            templates.AddRange(MSBE.Read(path).Parts.Enemies.Where(e=>e.ModelName=="c3200").Take(1));
        }
        foreach(var group in rows.GroupBy(r=>r[0])) {
            string name=group.Key;var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name+".msb.dcx"));
            var script=EMEVD.Read(Path.Combine(input,"event",name+".emevd.dcx"));
            var stone=map.Parts.Enemies.Single(e=>e.EntityID==uint.Parse(group.First()[2])/10000*10000+100);
            string collision=stone.CollisionPartName??map.Parts.Collisions.Single(c=>c.Name=="h001000").Name;
            foreach(var row in group) {
                uint id=uint.Parse(row[2]);string model=$"c{int.Parse(row[3]):D4}";
                if(map.Parts.GetEntries().Any(p=>p.EntityID==id)||script.Events.Any(e=>e.ID==id))throw new Exception("Service entity/event collision");
                foreach(var ins in script.Events.SelectMany(e=>e.Instructions))for(int at=0;at+4<=ins.ArgData.Length;at+=4)
                    if(BitConverter.ToUInt32(ins.ArgData,at)==id+70)throw new Exception("Service visibility flag collision");
                var part=(MSBE.Part.Enemy)templates.First(e=>e.ModelName==model).DeepCopy();
                part.EntityID=id;part.Name=$"tower_service_{id}";part.InstanceID=9700+(int)(id%100);
                if(map.Parts.Enemies.Any(e=>e.ModelName==model&&e.InstanceID==part.InstanceID))throw new Exception("Service instance collision");
                part.NPCParamID=int.Parse(row[4]);part.ThinkParamID=int.Parse(row[5]);part.CharaInitID=int.Parse(row[6]);
                part.Position=new(float.Parse(row[8],CultureInfo.InvariantCulture),float.Parse(row[9],CultureInfo.InvariantCulture),float.Parse(row[10],CultureInfo.InvariantCulture));
                var target=name.StartsWith("m45")?new Vector3(0,.1f,10):map.Regions.GetEntries().Single(r=>r.EntityID==id/10000*10000+2600).Position;
                part.Rotation=new(0,MathF.Atan2(part.Position.X-target.X,part.Position.Z-target.Z)*180/MathF.PI,0);
                part.TalkID=-1;part.EntityGroupIDs=new uint[8];part.WalkRouteName=null;part.ChrActivateCondParamID=0;part.BackupEventAnimID=-1;part.CollisionPartName=collision;
                if(!map.Models.GetEntries().Any(m=>m.Name==model)) {
                    if(model=="c0000")map.Models.Players.Add(new MSBE.Model.Player{Name=model});else map.Models.Enemies.Add(new MSBE.Model.Enemy{Name=model});
                }
                PartyPlacement.Prepare(part);
                map.Parts.Enemies.Add(part);script.Events.Add(Visibility(id));
                script.Events.Single(e=>e.ID==0).Instructions.Insert(0,new(2000,0,new object[]{0,id,0u}));
                script.Events.Single(e=>e.ID==0).Instructions.Insert(0,new(2003,66,new object[]{(byte)0,id+70,(byte)0}));
            }
            Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));Directory.CreateDirectory(Path.Combine(output,"event"));
            string mapPath=Path.Combine(output,"map/mapstudio",name+".msb.dcx"),eventPath=Path.Combine(output,"event",name+".emevd.dcx");
            PartyModels.Prepare(map);PartyModels.Verify(map);
            map.Write(mapPath);script.Write(eventPath);var checkedMap=MSBE.Read(mapPath);var checkedEvents=EMEVD.Read(eventPath);
            foreach(var row in group) {
                uint id=uint.Parse(row[2]);var actual=checkedMap.Parts.Enemies.Single(e=>e.EntityID==id);var original=map.Parts.Enemies.Single(e=>e.EntityID==id);
                PartyPlacement.Verify(actual);
                if(actual.NPCParamID!=original.NPCParamID||actual.CharaInitID!=original.CharaInitID||actual.Position!=original.Position||actual.CollisionPartName!=collision)throw new Exception("Service map roundtrip mismatch");
                var ev=checkedEvents.Events.Single(e=>e.ID==id);PersonalEvents.Verify(ev);
                if(ev.Instructions.Count!=Visibility(id).Instructions.Count||ev.Instructions.Zip(Visibility(id).Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Service event roundtrip mismatch");
            }
            Console.WriteLine($"Verified {group.Count()} conditional map services: {name}");
        }
    }
}
