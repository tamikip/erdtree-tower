using SoulsFormats;

static class EnemyExpansionAssets {
    static readonly (int act,int model)[] Added=[(1,4000),(1,4491),(2,4210),(2,4460)];
    public static void Verify(string input) {
        var rows=File.ReadAllLines(Path.Combine(input,"party-extra-slots.tsv")).Select(l=>l.Split('|').Select(uint.Parse).ToArray()).Where(p=>Added.Contains(((int)p[0],(int)p[2]))).ToArray();
        if(rows.Length!=12||rows.Select(p=>p[5]).Distinct().Count()!=12)throw new Exception("Missing or duplicate expansion slots");
        int checkedActors=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")) {
            var map=MSBE.Read(path);var eventPath=Path.Combine(input,"event",Path.GetFileName(path).Replace(".msb.dcx",".emevd.dcx"));
            if(!File.Exists(eventPath))continue;var events=EMEVD.Read(eventPath);
            foreach(var p in rows)foreach(bool guard in new[]{false,true}) {
                uint first=(guard?51000000u:48000000u)+p[0]*10000,id=guard?p[6]:p[5];
                if(!map.Parts.Enemies.Any(e=>e.EntityID>=first&&e.EntityID<first+300))continue;
                var part=map.Parts.Enemies.SingleOrDefault(e=>e.EntityID==id)??throw new Exception($"Missing expansion actor: {path}/{id}");
                if(part.NPCParamID!=p[1]||part.ThinkParamID!=p[3]||part.ModelName!=$"c{p[2]:D4}")throw new Exception("Expansion actor mismatch");
                PartyPlacement.Verify(part);
                foreach(bool battle in new[]{false,true}) {
                    var expected=guard?PartyGuardAssets.ForEntity(p[5],battle):battle?PartyEnemyEvents.Battle(id):PartyEnemyEvents.Create(id);
                    var ev=events.Events.Single(e=>e.ID==expected.ID);
                    if(ev.Instructions.Count!=expected.Instructions.Count||!ev.Instructions.Zip(expected.Instructions).All(pair=>pair.First.Bank==pair.Second.Bank&&pair.First.ID==pair.Second.ID&&pair.First.ArgData.SequenceEqual(pair.Second.ArgData)))throw new Exception("Expansion event mismatch");
                    if(!events.Events.Single(e=>e.ID==0).Instructions.Any(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==ev.ID))throw new Exception("Expansion event not initialized");
                }
                if(!guard)for(uint slot=0;slot<12;slot++)if(!map.Regions.GetEntries().Any(r=>r.EntityID==PartyEnemyEvents.Region(id,slot)))throw new Exception("Expansion placement region missing");
                checkedActors++;
            }
        }
        if(checkedActors<100)throw new Exception("Incomplete expansion arena coverage");
        Console.WriteLine($"Verified {checkedActors} expansion actors, both events and placement regions across all maps");
    }
    public static void English(string input,string output,string rules) {
        var names=new Dictionary<string,string>{{"死诞者","Revenant Follower"},{"小壶","Living Jar"},{"火焰战车","Flame Chariot"},{"战鹰","Warhawk"}};
        var translations=System.Text.Json.JsonSerializer.Deserialize<Dictionary<string,string>>(File.ReadAllText(rules))!;
        var pool=AffixTextFix.PoolNames(input);var previews=MelinaAssets.PreviewNames(input);
        foreach(var pack in new[]{"item","menu"}) {
            var relative=$"msg/engus/{pack}_dlc02.msgbnd.dcx";var binder=BND4.Read(Path.Combine(input,relative));
            var target=pack=="item"?binder.Files.First(f=>f.Name.Contains("NpcName")):binder.Files.Single(f=>f.ID==MelinaAssets.PreviewMenu);
            var fmg=FMG.Read(target.Bytes);var expected=new Dictionary<int,string>();
            foreach(var (name,en) in names) {
                if(pack=="item") {
                    int index=pool.IndexOf(name);if(index<0)throw new Exception("Missing enemy name");
                    expected[9610000+index]=en;
                    foreach(var line in File.ReadAllLines(Path.Combine(input,"boss-affix-combos.txt"))) {
                        var p=line.Split('|');expected[98000000+int.Parse(p[0])*256+index]=string.Join("·",p[1].Split('·').Select(a=>translations[a]))+" "+en;
                    }
                } else {int index=Array.IndexOf(previews,name);for(int n=1;n<=15;n++)expected[MelinaAssets.PreviewFirst+index*16+n]=$"{en} ×{n}";}
            }
            var original=fmg.Entries.Where(e=>!expected.ContainsKey(e.ID)).Select(e=>(e.ID,e.Text)).ToArray();
            fmg.Entries.RemoveAll(e=>expected.ContainsKey(e.ID));foreach(var (id,text) in expected)fmg.Entries.Add(new FMG.Entry(id,text));
            fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));target.Bytes=fmg.Write();
            var path=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(path)!);binder.Write(path);
            var check=BND4.Read(path);if(check.Files.Any(f=>!f.Bytes.SequenceEqual(binder.Files.Single(b=>b.ID==f.ID).Bytes)))throw new Exception("English text roundtrip failed");
            var reread=FMG.Read(check.Files.Single(f=>f.ID==target.ID).Bytes);
            if(!original.SequenceEqual(reread.Entries.Where(e=>!expected.ContainsKey(e.ID)).Select(e=>(e.ID,e.Text))))throw new Exception("Unrelated English text changed");
            Console.WriteLine($"Verified {expected.Count} English enemy {pack} entries");
        }
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var sources=Enumerable.Range(1,3).ToDictionary(a=>a,a=>MSBE.Read(Path.Combine(input,"map/mapstudio",$"m45_0{3-a}_00_00.msb.dcx")));
        var actors=Added.SelectMany(p=>sources[p.act].Parts.Enemies.Where(e=>ArenaPartyAssets.Combat(e.EntityID)&&e.NPCParamID==900000000+p.model*10000).Select(e=>(p.act,part:e))).ToArray();
        if(actors.Length!=12)throw new Exception("Expected 12 new source slots");
        int maps=0,total=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")) {
            var map=MSBE.Read(path);var name=Path.GetFileName(path).Replace(".msb.dcx","");
            var eventPath=Path.Combine(input,"event",name+".emevd.dcx");if(!File.Exists(eventPath))continue;
            var events=EMEVD.Read(eventPath);var originalMap=map.Write();var originalEvents=events.Write();
            var ids=new HashSet<uint>();var modelNames=new HashSet<string>();var eventIds=new HashSet<long>();var regionNames=new HashSet<string>();int prefix=0;
            var init=events.Events.Single(e=>e.ID==0);
            foreach(var (act,source) in actors)foreach(bool guard in new[]{false,true}) {
                uint id=source.EntityID+(guard?3000000u:0u),first=(uint)((guard?51000000:48000000)+act*10000);
                var anchor=map.Parts.Enemies.FirstOrDefault(e=>e.EntityID>=first&&e.EntityID<first+300);
                if(anchor==null)continue;
                var present=map.Parts.Enemies.SingleOrDefault(e=>e.EntityID==id);
                if(present!=null){if(present.NPCParamID!=source.NPCParamID||present.ThinkParamID!=source.ThinkParamID||present.ModelName!=source.ModelName)throw new Exception("Existing enemy mismatch");continue;}
                if(!guard)for(uint slot=0;slot<12;slot++) {
                    var regionId=PartyEnemyEvents.Region(id,slot);if(map.Regions.GetEntries().Any(r=>r.EntityID==regionId))continue;
                    var region=map.Regions.GetEntries().Single(r=>r.EntityID==PartyEnemyEvents.Region((uint)(48000000+act*10000),slot)).DeepCopy();
                    region.EntityID=regionId;region.Name="tower_expansion_position_"+regionId;region.ActivationPartName=null;map.Regions.Add(region);regionNames.Add(region.Name);
                }
                var part=(MSBE.Part.Enemy)source.DeepCopy();part.EntityID=id;part.Name=guard?$"tower_guard_{id}":$"tower_party_actor_{id}";
                part.InstanceID=map.Parts.Enemies.Where(e=>e.ModelName==part.ModelName).Select(e=>e.InstanceID).DefaultIfEmpty(-1).Max()+1;
                part.Position=anchor.Position;part.Rotation=anchor.Rotation;part.CollisionPartName=anchor.CollisionPartName;
                part.WalkRouteName=null;part.PlatoonID=-1;PartyPlacement.Prepare(part);
                if(!map.Models.Enemies.Any(m=>m.Name==part.ModelName)){map.Models.Enemies.Add((MSBE.Model.Enemy)sources[act].Models.Enemies.Single(m=>m.Name==part.ModelName).DeepCopy());modelNames.Add(part.ModelName);}
                map.Parts.Enemies.Add(part);ids.Add(id);
                init.Instructions.Insert(prefix++,new(2004,5,new object[]{id,(byte)0}));
                uint request=guard?45003000+(source.EntityID/10000-4801)*300+source.EntityID%10000:PartyEnemyEvents.Request(id);
                init.Instructions.Insert(prefix++,new(2003,66,new object[]{(byte)0,request,(byte)0}));
                if(!guard)init.Instructions.Insert(prefix++,new(2003,66,new object[]{(byte)0,PartyEnemyEvents.Ready(id),(byte)0}));
                foreach(bool battle in new[]{false,true}) {
                    var ev=guard?PartyGuardAssets.ForEntity(source.EntityID,battle):battle?PartyEnemyEvents.Battle(id):PartyEnemyEvents.Create(id);
                    if(events.Events.Any(e=>e.ID==ev.ID))throw new Exception("Enemy event collision");
                    events.Events.Add(ev);eventIds.Add(ev.ID);init.Instructions.Insert(prefix++,new(2000,0,new object[]{0,(uint)ev.ID,0u}));
                }
            }
            if(ids.Count==0)continue;
            var mapOut=Path.Combine(output,"map/mapstudio",name+".msb.dcx");var eventOut=Path.Combine(output,"event",name+".emevd.dcx");
            Directory.CreateDirectory(Path.GetDirectoryName(mapOut)!);Directory.CreateDirectory(Path.GetDirectoryName(eventOut)!);
            PartyModels.Prepare(map);PartyModels.Verify(map);map.Write(mapOut);events.Write(eventOut);
            var check=MSBE.Read(mapOut);var script=EMEVD.Read(eventOut);
            foreach(var id in ids){var p=check.Parts.Enemies.Single(e=>e.EntityID==id);PartyPlacement.Verify(p);if(!check.Parts.Collisions.Any(c=>c.Name==p.CollisionPartName))throw new Exception("Enemy ground missing");}
            if(!script.Write().SequenceEqual(events.Write()))throw new Exception("Enemy event roundtrip mismatch");
            check.Parts.Enemies.RemoveAll(e=>ids.Contains(e.EntityID));check.Models.Enemies.RemoveAll(m=>modelNames.Contains(m.Name));
            check.Regions.InvasionPoints.RemoveAll(r=>regionNames.Contains(r.Name));
            script.Events.RemoveAll(e=>eventIds.Contains(e.ID));script.Events.Single(e=>e.ID==0).Instructions.RemoveRange(0,prefix);
            if(!check.Write().SequenceEqual(originalMap)||!script.Write().SequenceEqual(originalEvents))throw new Exception($"Unrelated arena changed: {name}");
            total+=ids.Count;maps++;
        }
        MusicAssets.RefreshArenaManifest(input,output);
        Console.WriteLine($"Added {total} enemy/guard placements across {maps} maps; existing map and event data preserved");
    }
}
