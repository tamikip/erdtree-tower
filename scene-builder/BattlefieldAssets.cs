using SoulsFormats;
using System.Numerics;
using System.Globalization;

static class BattlefieldAssets {
    static string[][] Rows()=>File.ReadAllLines("outputs/erdtree-tower/scene-assets/battlefields.txt").Select(l=>l.Split('|')).ToArray();
    static Vector3 Point(string[] row,int offset)=>new(float.Parse(row[offset],CultureInfo.InvariantCulture),float.Parse(row[offset+1],CultureInfo.InvariantCulture),float.Parse(row[offset+2],CultureInfo.InvariantCulture));
    public static void Build(string output) {
        foreach(var row in Rows()) {
            string name=row[1];uint basis=uint.Parse(row[2]);
            float yaw=float.Parse(row[7],CultureInfo.InvariantCulture);
            var map=MSBE.Read(Path.Combine("work/boss-report/maps",name+".msb.dcx"));
            var anchor=map.Parts.Enemies.Single(e=>e.Name==row[14]);
            var templateName=$"m45_0{(basis-45000000)/10000}_00_00";
            var template=MSBE.Read(Path.Combine(output,"map/mapstudio",templateName+".msb.dcx"));
            var events=EMEVD.Read(Path.Combine(output,"event",templateName+".emevd.dcx"));
            events.Events=events.Events.Where(e=>new long[]{basis+983,basis+984,basis+985,basis+986}.Contains(e.ID)).ToList();
            var init=new EMEVD.Event(0);events.Events.Insert(0,init);
            foreach(var ev in events.Events.Skip(1))init.Instructions.Add(new EMEVD.Instruction(2000,0,new object[]{0,(uint)ev.ID,0u}));
            uint assigned=49000000;
            var used=map.Parts.GetEntries().Select(p=>p.EntityID).ToHashSet();
            foreach(var enemy in map.Parts.Enemies) {
                if(enemy.EntityID==0||enemy.EntityID==uint.MaxValue) {
                    while(used.Contains(assigned))assigned++;
                    enemy.EntityID=assigned++;used.Add(enemy.EntityID);
                }
                init.Instructions.Add(new EMEVD.Instruction(2004,5,new object[]{enemy.EntityID,(byte)0}));
            }
            var stone=(MSBE.Part.Enemy)template.Parts.Enemies.Single(e=>e.EntityID==basis+100).DeepCopy();
            stone.Name="tower_summon_stone";stone.Position=Point(row,4);stone.CollisionPartName=anchor.CollisionPartName;stone.EntityGroupIDs=new uint[8];stone.TalkID=2000;
            if(map.Parts.GetEntries().Any(p=>p.EntityID==stone.EntityID))throw new Exception("Battlefield stone collision");
            if(!map.Models.Enemies.Any(m=>m.Name==stone.ModelName))map.Models.Enemies.Add((MSBE.Model.Enemy)template.Models.Enemies.Single(m=>m.Name==stone.ModelName).DeepCopy());
            map.Parts.Enemies.Add(stone);
            var entry=template.Regions.GetEntries().Single(r=>r.EntityID==basis+2600).DeepCopy();
            entry.Name="tower_entry";entry.ActivationPartName=null;entry.Position=Point(row,8);entry.Rotation=new Vector3(0,yaw,0);map.Regions.Add(entry);
            foreach(var player in map.Parts.Players){player.Position=entry.Position;player.Rotation=entry.Rotation;}
            var portal=(MSBE.Part.Asset)template.Parts.Assets.Single(a=>a.EntityID==basis+1900).DeepCopy();
            portal.Position=Point(row,11);portal.Rotation=new Vector3(0,yaw+180,0);
            if(!map.Models.Assets.Any(m=>m.Name==portal.ModelName))map.Models.Assets.Add((MSBE.Model.Asset)template.Models.Assets.Single(m=>m.Name==portal.ModelName).DeepCopy());
            map.Parts.Assets.Add(portal);
            BossAssets.Events(events,basis,output);
            map.Write(Path.Combine(output,"map/mapstudio",name+".msb.dcx"));
            events.Write(Path.Combine(output,"event",name+".emevd.dcx"));
            Console.WriteLine($"Built distinct battlefield: {row[3]} ({name})");
        }
        var source=Path.GetFullPath("outputs/erdtree-tower/scene-assets/battlefields.txt");var target=Path.GetFullPath(Path.Combine(output,"battlefields.txt"));
        if(source!=target)File.Copy(source,target,true);
        Verify(output);
    }
    public static void Verify(string output) {
        foreach(var row in Rows()) {
            uint basis=uint.Parse(row[2]);
            var map=MSBE.Read(Path.Combine(output,"map/mapstudio",row[1]+".msb.dcx"));
            var events=EMEVD.Read(Path.Combine(output,"event",row[1]+".emevd.dcx"));
            if(row[1]=="m10_00_00_00"&&events.Events.Any(e=>e.ID==10007980)){StormveilAssets.Verify(output);continue;}
            var stone=map.Parts.Enemies.Single(e=>e.EntityID==basis+100);
            if(stone.TalkID!=2000||stone.Position!=Point(row,4)||!map.Parts.Collisions.Any(c=>c.Name==stone.CollisionPartName))throw new Exception("Invalid battlefield summon stone");
            if(map.Regions.GetEntries().Single(r=>r.EntityID==basis+2600).Position!=Point(row,8)||map.Parts.Assets.Single(a=>a.EntityID==basis+1900).Position!=Point(row,11))throw new Exception("Invalid battlefield entry/portal");
            var init=events.Events.Single(e=>e.ID==0);
            foreach(var enemy in map.Parts.Enemies.Where(e=>e.EntityID!=basis+100))if(!init.Instructions.Any(i=>i.Bank==2004&&i.ID==5&&BitConverter.ToUInt32(i.ArgData,0)==enemy.EntityID&&i.ArgData[4]==0))throw new Exception("Vanilla battlefield enemy not disabled");
            if(events.Events.Count(e=>e.ID>=basis+3000&&e.ID<basis+4000)!=BossAssets.Parts(output).Length*3)throw new Exception("Missing battlefield boss bars");
        }
        Console.WriteLine("Verified distinct battlefields, original enemy suppression, entries, portals and summon stones");
    }
}
