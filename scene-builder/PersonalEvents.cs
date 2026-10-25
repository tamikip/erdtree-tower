using SoulsFormats;

static class PersonalEvents {
    public static void Local(EMEVD.Event ev) {
        if(ev.Instructions.Any(i=>i.Bank==2000&&i.ID==2))throw new Exception("Event already declares network synchronization");
        ev.Instructions.Insert(0,new EMEVD.Instruction(2000,2,new object[]{(byte)0}));
    }
    public static void Verify(EMEVD.Event ev) {
        var first=ev.Instructions.FirstOrDefault();
        if(first==null||first.Bank!=2000||first.ID!=2||first.ArgData.Length!=4||first.ArgData[0]!=0)
            throw new Exception($"Personal event {ev.ID} must disable network synchronization first");
        if(ev.Instructions.Any(i=>i.Bank==2003&&(i.ID==63||i.ID==69)))
            throw new Exception($"Personal event {ev.ID} writes network-connected flags");
    }
    public static void Test() {
        var file=new EMEVD();
        var ev=new EMEVD.Event(45000982,EMEVD.Event.RestBehaviorType.Restart);
        ev.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,45000994u,(byte)1}));
        Local(ev);file.Events.Add(ev);
        var copy=EMEVD.Read(file.Write());Verify(copy.Events.Single());
        if(copy.Events.Single().Instructions.Count!=2)throw new Exception("Personal event roundtrip lost instructions");
        var invalid=new EMEVD.Event(1,EMEVD.Event.RestBehaviorType.Restart);Local(invalid);
        invalid.Instructions.Add(new EMEVD.Instruction(2003,69,new object[]{(byte)0,45000994u,(byte)1}));
        try {Verify(invalid);} catch(Exception) {Console.WriteLine("Personal event roundtrip and network-write rejection passed");return;}
        throw new Exception("Network-connected flag write was accepted");
    }
    public static void Patch(string input,string output) {
        Directory.CreateDirectory(Path.Combine(output,"event"));
        for(int map=0;map<3;map++) {
            var name=$"m45_0{map}_00_00.emevd.dcx";
            var file=EMEVD.Read(Path.Combine(input,"event",name));
            long basis=45000980+map*10000;
            var ids=map==1?new[]{basis,basis+1,basis+2,basis+3,basis+5,basis+6}:new[]{basis,basis+2,basis+3,basis+5,basis+6};
            if(map==1)ids=ids.Concat(file.Events.Where(e=>e.ID>=45010800&&e.ID<=45010843).Select(e=>e.ID)).ToArray();
            PatchFile(file,Path.Combine(output,"event",name),ids);
        }
        foreach(var line in File.ReadAllLines(Path.Combine(input,"battlefields.txt"))) {
            var row=line.Split('|');long basis=long.Parse(row[2]);
            var name=row[1]+".emevd.dcx";
            PatchFile(EMEVD.Read(Path.Combine(input,"event",name)),Path.Combine(output,"event",name),new[]{basis+983,basis+985,basis+986});
        }
    }
    static void PatchFile(EMEVD file,string path,long[] ids) {
        var originalCount=file.Events.Count;
        foreach(var id in ids) {
            var ev=file.Events.Single(e=>e.ID==id);
            if(!ev.Instructions.Any(i=>i.Bank==2000&&i.ID==2))Local(ev);
            Verify(ev);
        }
        file.Write(path);var copy=EMEVD.Read(path);
        if(copy.Events.Count!=originalCount)throw new Exception("Event count changed during personal event patch");
        foreach(var id in ids)Verify(copy.Events.Single(e=>e.ID==id));
        Console.WriteLine($"Verified local events: {Path.GetFileName(path)} ({ids.Length})");
    }
}
