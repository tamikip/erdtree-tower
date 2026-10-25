using SoulsFormats;

static class PartyCompletionEvents {
    public static EMEVD.Event Banner(long id) {
        var ev=new EMEVD.Event(id,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,45020972u}));
        ev.Instructions.Add(new(2007,2,new object[]{(byte)18}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,45020972u,(byte)0}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));
        return ev;
    }
    public static void Verify(EMEVD.Event ev) {
        PersonalEvents.Verify(ev);
        var expected=Banner(ev.ID);
        if(ev.Instructions.Count!=expected.Instructions.Count||ev.Instructions.Zip(expected.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Victory banner must consume one local pulse, independent of portal state");
    }
    public static void Patch(string input,string output) {
        for(int map=0;map<3;map++) {
            var name=$"m45_0{map}_00_00.emevd.dcx";var file=EMEVD.Read(Path.Combine(input,"event",name));
            long id=45000984+map*10000;var ev=file.Events.Single(e=>e.ID==id);
            file.Events[file.Events.IndexOf(ev)]=Banner(id);
            Directory.CreateDirectory(Path.Combine(output,"event"));var path=Path.Combine(output,"event",name);
            file.Write(path);Verify(EMEVD.Read(path).Events.Single(e=>e.ID==id));
        }
    }
}
