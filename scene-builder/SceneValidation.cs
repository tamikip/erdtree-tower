using SoulsFormats;
using System.Numerics;

static class SceneValidation {
    public static void Verify(string output) {
        PartyCompletionEvents.Verify(EMEVD.Read(Path.Combine(output,"event","m45_02_00_00.emevd.dcx")).Events.Single(e=>e.ID==45020984));
        foreach(uint basis in new[]{45000000u,45010000u,45020000u}) {
            var map=$"m45_0{(basis-45000000)/10000}_00_00";
            var events=EMEVD.Read(Path.Combine(output,"event",map+".emevd.dcx"));
            var visual=events.Events.SingleOrDefault(e=>e.ID==basis+986) ?? throw new Exception("Missing service portal activation visual");
            bool HasFlag(uint flag,byte state)=>visual.Instructions.Any(i=>i.Bank==3&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==flag&&i.ArgData[1]==state);
            foreach(var flag in basis==45010000?new[]{basis+993,basis+995,45020970u}:new[]{basis+993,45020970u})
                if(!HasFlag(flag,0)||!HasFlag(flag,1))throw new Exception("Portal visual must follow both activation and deactivation conditions");
            if(visual.Instructions.Count(i=>i.Bank==2006&&i.ID==4)!=1||visual.Instructions.Count(i=>i.Bank==2006&&i.ID==5)!=1||visual.Instructions.Any(i=>i.Bank==2007))throw new Exception("Portal visual must not replay victory banners");
            if(!events.Events.Single(e=>e.ID==0).Instructions.Any(i=>i.Bank==2000&&i.ID==0&&BitConverter.ToUInt32(i.ArgData,4)==basis+986))throw new Exception("Portal visual event is not initialized");
            for(int mask=0;mask<8;mask++) {
                var flags=new Dictionary<uint,bool>{{basis+993,(mask&1)!=0},{basis+995,(mask&2)!=0},{45020970u,(mask&4)!=0}};
                bool expected=flags[45020970]||(flags[basis+993]&&(basis!=45010000||flags[basis+995]));
                var waits=EvaluateWaits(visual,flags);
                if(waits.Count!=2||waits[0]!=expected||waits[1]==expected)throw new Exception("Portal visual activation truth table mismatch");
            }
            var msb=MSBE.Read(Path.Combine(output,"map","mapstudio",map+".msb.dcx"));
            var portal=msb.Parts.Assets.Single(a=>a.EntityID==basis+1900);
            if(msb.Parts.Players.Count==0||msb.Parts.Players.Any(p=>p.Position.X!=0||p.Position.Z!=10||p.Rotation.Y!=0))throw new Exception("Default player spawn must stay opposite the portal too");
            foreach(var id in basis==45020000?new[]{basis+2600,basis+2710}:new[]{basis+2600}) {
                var entry=msb.Regions.GetEntries().Single(r=>r.EntityID==id);
                if(entry.Position.X!=0||entry.Position.Z!=10||entry.Rotation.Y!=0||portal.Position.X!=0||portal.Position.Z!=-12||portal.Rotation.Y!=180)throw new Exception("Player/portal must face each other across arena center");
            }
        }
        var chestMap=MSBE.Read(Path.Combine(output,"map","mapstudio","m45_01_00_00.msb.dcx"));
        var chest=chestMap.Parts.Assets.Single(a=>a.EntityID==45011901);
        if(chest.Position.Z!=2||chest.Position.X!=0||chest.Rotation.Y!=180)throw new Exception("Chest must face the entry with a clear approach");
        foreach(var box in chestMap.Parts.Assets.Where(a=>a.EntityID>=45011910&&a.EntityID<=45011912))
            if(box.Rotation.Y!=180)throw new Exception("Event chest opening must face the entry");
        var chestEvent=EMEVD.Read(Path.Combine(output,"event","m45_01_00_00.emevd.dcx")).Events.Single(e=>e.ID==45010981);
        PersonalChestEvents.Verify(chestEvent);var sequence=chestEvent.Instructions;
        int animation=sequence.FindIndex(i=>i.Bank==2003&&i.ID==18);
        if(!sequence.Any(i=>i.Bank==2004&&i.ID==14&&BitConverter.ToInt32(i.ArgData,8)==60080))throw new Exception("Chest must use the native lid-opening player animation");
        if(animation<0||sequence[animation].ArgData[9]!=1)throw new Exception("Chest must wait for the opening animation");
        if(!sequence.Any(i=>i.Bank==3&&i.ID==24&&BitConverter.ToInt32(i.ArgData,4)==9500002)||!sequence.Any(i=>i.Bank==0&&i.ID==0))throw new Exception("Chest must require active room, unclaimed reward and interaction together");
        Console.WriteLine("Verified centered entries/portals, chest facing and one-shot native reward sequence");
    }
    static List<bool> EvaluateWaits(EMEVD.Event ev,Dictionary<uint,bool> flags) {
        var groups=new Dictionary<sbyte,bool>();var waits=new List<bool>();
        foreach(var i in ev.Instructions) {
            bool value;
            if(i.Bank==3&&i.ID==0)value=flags[BitConverter.ToUInt32(i.ArgData,4)]==(i.ArgData[1]!=0);
            else if(i.Bank==0&&i.ID==0)value=groups[(sbyte)i.ArgData[2]]==(i.ArgData[1]!=0);
            else continue;
            var group=(sbyte)i.ArgData[0];
            if(group==0){waits.Add(value);groups.Clear();}
            else groups[group]=group>0?groups.GetValueOrDefault(group,true)&&value:groups.GetValueOrDefault(group,false)||value;
        }
        return waits;
    }
}
