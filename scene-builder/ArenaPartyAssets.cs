using SoulsFormats;
using System.Numerics;

static class ArenaPartyAssets {
    public static bool Combat(uint id)=>id>=48010000&&id<48040000&&id%10000<300;
    public static void Add(string input,MSBE target,EMEVD events,int act,MSBE.Part.Enemy anchor,Func<Vector3,Vector3> place,float yaw) {
        var source=MSBE.Read(Path.Combine(input,"map/mapstudio",$"m45_0{3-act}_00_00.msb.dcx"));
        uint first=(uint)(48000000+act*10000);
        var enemies=source.Parts.Enemies.Where(e=>e.EntityID>=first&&e.EntityID<first+300).ToArray();
        if(enemies.Length==0)throw new Exception("No party enemies for act "+act);
        var init=events.Events.Single(e=>e.ID==0);
        var bootstrap=new List<EMEVD.Instruction>{new(2003,66,new object[]{(byte)0,PartyEnemyEvents.Combat,(byte)0})};
        foreach(var enemy in enemies) {
            bootstrap.Add(new(2004,5,new object[]{enemy.EntityID,(byte)0}));
            bootstrap.Add(new(2003,66,new object[]{(byte)0,PartyEnemyEvents.Request(enemy.EntityID),(byte)0}));
            bootstrap.Add(new(2003,66,new object[]{(byte)0,PartyEnemyEvents.Ready(enemy.EntityID),(byte)0}));
        }
        for(uint slot=0;slot<12;slot++) {
            var original=source.Regions.GetEntries().Single(r=>r.EntityID==PartyEnemyEvents.Region(first,slot));
            var region=original.DeepCopy();region.Name=$"tower_party_position_{act}_{slot}";region.ActivationPartName=null;
            region.Position=place(original.Position);region.Rotation=new(original.Rotation.X,original.Rotation.Y+yaw,original.Rotation.Z);
            if(!target.Regions.GetEntries().Any(r=>r.EntityID==region.EntityID))target.Regions.Add(region);
        }
        foreach(var original in enemies) {
            var enemy=(MSBE.Part.Enemy)original.DeepCopy();
            enemy.Name="tower_party_actor_"+enemy.EntityID;
            if(target.Parts.GetEntries().Any(p=>p.EntityID==enemy.EntityID))throw new Exception("Party actor ID collision");
            enemy.InstanceID=target.Parts.Enemies.Where(p=>p.ModelName==enemy.ModelName).Select(p=>p.InstanceID).DefaultIfEmpty(-1).Max()+1;
            enemy.Position=place(original.Position);enemy.Rotation=new(original.Rotation.X,original.Rotation.Y+yaw,original.Rotation.Z);
            enemy.CollisionPartName=anchor.CollisionPartName;enemy.WalkRouteName=null;enemy.PlatoonID=-1;PartyPlacement.Prepare(enemy);
            if(!target.Models.Enemies.Any(m=>m.Name==enemy.ModelName))target.Models.Enemies.Add((MSBE.Model.Enemy)source.Models.Enemies.Single(m=>m.Name==enemy.ModelName).DeepCopy());
            target.Parts.Enemies.Add(enemy);
            foreach(var ev in new[]{PartyEnemyEvents.Create(enemy.EntityID),PartyEnemyEvents.Battle(enemy.EntityID)}) {
                if(events.Events.Any(e=>e.ID==ev.ID))throw new Exception("Party event ID collision");
                events.Events.Add(ev);bootstrap.Add(new(2000,0,new object[]{0,(uint)ev.ID,0u}));
            }
        }
        init.Instructions.InsertRange(0,bootstrap);
        PartyModels.Prepare(target);Verify(target,events,source,act);
        if(act==1)FirstActEliteAssets.Add(input,target,events);
        if(act==2)SecondActEliteAssets.Add(input,target,events);
        if(act==3)ThirdActEliteAssets.Add(input,target,events);
    }
    public static void Verify(MSBE target,EMEVD events,MSBE source,int act) {
        uint first=(uint)(48000000+act*10000);
        var expected=source.Parts.Enemies.Where(e=>e.EntityID>=first&&e.EntityID<first+300).ToArray();
        var init=events.Events.Single(e=>e.ID==0).Instructions;
        int branch=init.FindIndex(i=>i.Bank<2000);if(branch<0)branch=init.Count;
        foreach(var enemy in expected) {
            int disabled=init.FindIndex(i=>i.Bank==2004&&i.ID==5&&BitConverter.ToUInt32(i.ArgData,0)==enemy.EntityID&&i.ArgData[4]==0);
            if(disabled<0||disabled>=branch)throw new Exception($"Party actor {enemy.EntityID} is not disabled before constructor control flow");
            foreach(uint id in new[]{enemy.EntityID,enemy.EntityID+100000}) {
                int start=init.FindIndex(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==id);
                if(start<=disabled||start>=branch)throw new Exception($"Party actor event {id} is not initialized before constructor control flow");
            }
        }
        if(target.Parts.Enemies.Count(e=>e.EntityID>=first&&e.EntityID<first+300&&!FirstActEliteAssets.Slot(e.EntityID))!=expected.Length)throw new Exception("Party roster count mismatch");
        foreach(var original in expected) {
            var actual=target.Parts.Enemies.Single(e=>e.EntityID==original.EntityID);
            if(actual.ModelName!=original.ModelName||actual.NPCParamID!=original.NPCParamID||actual.ThinkParamID!=original.ThinkParamID)throw new Exception("Party actor data mismatch");
            PartyPlacement.Verify(actual);
            if(!target.Parts.Collisions.Any(c=>c.Name==actual.CollisionPartName))throw new Exception("Party actor collision missing");
            foreach(var id in new[]{actual.EntityID,actual.EntityID+100000})PartyEnemyEvents.Verify(events.Events.Single(e=>e.ID==id),actual.EntityID);
        }
        for(uint slot=0;slot<12;slot++)if(target.Regions.GetEntries().Count(r=>r.EntityID==PartyEnemyEvents.Region(first,slot))!=1)throw new Exception("Party formation region missing");
    }
}
