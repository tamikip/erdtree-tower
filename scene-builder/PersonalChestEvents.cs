using SoulsFormats;

static class PersonalChestEvents {
    public static EMEVD.Instruction[] VisibilityInstructions()=>new EMEVD.Instruction[]{
        new(2005,3,new object[]{45011901u,(byte)0}),
        new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,45010939u}),
        new(3,0,new object[]{(sbyte)1,(byte)0,(byte)0,45010940u}),
        new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}),
        new(2005,3,new object[]{45011901u,(byte)1}),
        new(3,0,new object[]{(sbyte)-1,(byte)0,(byte)0,45010939u}),
        new(3,0,new object[]{(sbyte)-1,(byte)1,(byte)0,45010940u}),
        new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)-1}),
        new(1000,4,new object[]{(byte)1})};
    public static EMEVD.Event Create() {
        var ev=new EMEVD.Event(45010981,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,45010993u}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,45010939u}));
        foreach(uint flag in new[]{45010995u,45010989u,45010940u})ev.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)0,(byte)0,flag}));
        ev.Instructions.Add(new(3,24,new object[]{(sbyte)1,9500002,45011901u}));
        ev.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,45010989u,(byte)1}));
        ev.Instructions.Add(new(2004,14,new object[]{10000u,45011901u,60080,(byte)0}));
        ev.Instructions.Add(new(1001,0,new object[]{0.25f}));
        ev.Instructions.Add(new(2003,18,new object[]{45011901u,1,(byte)0,(byte)1,(byte)0,(sbyte)0,1f}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,45010990u,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,45010993u}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));return ev;
    }
    public static void Verify(EMEVD.Event ev) {
        PersonalEvents.Verify(ev);var steps=ev.Instructions;
        if(!steps.Any(i=>i.Bank==3&&i.ID==0&&i.ArgData[0]==1&&i.ArgData[1]==1&&BitConverter.ToUInt32(i.ArgData,4)==45010939))throw new Exception("Chest interaction must require a chest node");
        int lockAt=steps.FindIndex(i=>i.Bank==2003&&i.ID==66&&BitConverter.ToUInt32(i.ArgData,4)==45010989);
        int animationAt=steps.FindIndex(i=>i.Bank==2003&&i.ID==18);
        int requestAt=steps.FindIndex(i=>i.Bank==2003&&i.ID==66&&BitConverter.ToUInt32(i.ArgData,4)==45010990);
        if(lockAt<0||animationAt<=lockAt||requestAt<=animationAt||steps.Any(i=>i.Bank==2003&&i.ID==4))throw new Exception("Chest must lock before animation and request a personal reward afterward; shared lot awards are forbidden");
        if(!steps.Any(i=>i.Bank==3&&i.ID==0&&i.ArgData[0]==1&&i.ArgData[1]==0&&BitConverter.ToUInt32(i.ArgData,4)==45010989))throw new Exception("Chest interaction must reject repeated opening");
    }
    public static void Patch(string input,string output) {
        const string name="m45_01_00_00.emevd.dcx";
        var file=EMEVD.Read(Path.Combine(input,"event",name));
        var untouched=file.Events.Where(e=>e.ID!=45010981&&e.ID!=45010800&&e.ID!=45010980).Select(e=>(e.ID,Steps:string.Join(";",e.Instructions.Select(i=>$"{i.Bank}:{i.ID}:{Convert.ToHexString(i.ArgData)}")))).ToArray();
        var visibility=file.Events.Single(e=>e.ID==45010800);
        visibility.Instructions.Clear();PersonalEvents.Local(visibility);visibility.Instructions.AddRange(VisibilityInstructions());
        var old=file.Events.Single(e=>e.ID==45010981);file.Events[file.Events.IndexOf(old)]=Create();
        var initialize=file.Events.Single(e=>e.ID==45010980);
        if(!initialize.Instructions.Any(i=>i.Bank==2003&&i.ID==66&&BitConverter.ToUInt32(i.ArgData,4)==45010989))
            initialize.Instructions.Insert(initialize.Instructions.Count-2,new(2003,66,new object[]{(byte)0,45010989u,(byte)0}));
        Directory.CreateDirectory(Path.Combine(output,"event"));string path=Path.Combine(output,"event",name);file.Write(path);
        Verify(EMEVD.Read(path).Events.Single(e=>e.ID==45010981));
        var check=EMEVD.Read(path);
        var visible=check.Events.Single(e=>e.ID==45010800);PersonalEvents.Verify(visible);
        if(!visible.Instructions.Skip(1).Select(i=>$"{i.Bank}:{i.ID}:{Convert.ToHexString(i.ArgData)}").SequenceEqual(VisibilityInstructions().Select(i=>$"{i.Bank}:{i.ID}:{Convert.ToHexString(i.ArgData)}")))throw new Exception("Chest visibility gate mismatch");
        foreach(var item in untouched)if(item.Steps!=string.Join(";",check.Events.Single(e=>e.ID==item.ID).Instructions.Select(i=>$"{i.Bank}:{i.ID}:{Convert.ToHexString(i.ArgData)}")))throw new Exception("Unrelated event changed");
        Console.WriteLine("Verified personal chest lock, animation and reward request sequence");
    }
    public static void Test(string input) {
        var current=EMEVD.Read(Path.Combine(input,"event","m45_01_00_00.emevd.dcx"));Verify(current.Events.Single(e=>e.ID==45010981));
    }
}
