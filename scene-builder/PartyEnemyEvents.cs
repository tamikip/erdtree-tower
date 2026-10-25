using SoulsFormats;

static class PartyEnemyEvents {
    public const uint Combat=45007998;
    static uint Offset(uint entity) {
        if(entity<48010000||entity>=48040000||(entity-48000000)%10000>=300)throw new ArgumentOutOfRangeException(nameof(entity));
        return ((entity-48000000)/10000-1)*300+(entity-48000000)%10000;
    }
    public static uint Request(uint entity)=>45008000+Offset(entity);
    public static uint Ready(uint entity)=>45009000+Offset(entity);
    public static uint Position(uint entity) {Offset(entity);return 45004000+((entity-48000000)/10000-1)*10000+(entity%10000)*4;}
    public static uint Region(uint entity,uint slot) {Offset(entity);if(slot>=12)throw new ArgumentOutOfRangeException(nameof(slot));return entity/10000*10000+9000+slot;}
    public static EMEVD.Event Create(uint entity) {
        uint request=Request(entity),ready=Ready(entity);
        var ev=new EMEVD.Event(entity,EMEVD.Event.RestBehaviorType.Restart);
        PersonalEvents.Local(ev);
        ev.Instructions.Add(new(2004,1,new object[]{entity,(byte)0}));
        ev.Instructions.Add(new(2004,15,new object[]{entity,(byte)1}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,ready,(byte)0}));
        ev.Instructions.Add(new(2004,5,new object[]{entity,(byte)0}));
        ev.Instructions.Add(new(2004,29,new object[]{entity,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,request}));
        ev.Instructions.Add(new(2004,35,new object[]{entity,(byte)1}));
        ev.Instructions.Add(new(2004,29,new object[]{entity,(byte)0}));
        ev.Instructions.Add(new(2004,5,new object[]{entity,(byte)1}));
        ev.Instructions.Add(new(4,7,new object[]{(sbyte)0,entity,(byte)1,(sbyte)0,1f}));
        for(uint slot=0;slot<12;slot++) {
            sbyte group=(sbyte)(slot+1);
            for(uint bit=0;bit<4;bit++)ev.Instructions.Add(new(3,0,new object[]{group,(byte)((slot>>((int)bit))&1),(byte)0,Position(entity)+bit}));
            ev.Instructions.Add(new(1000,1,new object[]{(byte)1,(byte)0,group}));
            ev.Instructions.Add(new(2004,42,new object[]{entity,(byte)1,Region(entity,slot),-1,10000u}));
        }
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,ready,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,request}));
        ev.Instructions.Add(new(2004,5,new object[]{entity,(byte)0}));
        ev.Instructions.Add(new(2003,66,new object[]{(byte)0,ready,(byte)0}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));
        return ev;
    }
    public static EMEVD.Event Battle(uint entity) {
        uint request=Request(entity);
        var ev=new EMEVD.Event(entity+100000,EMEVD.Event.RestBehaviorType.Restart);
        PersonalEvents.Local(ev);
        ev.Instructions.Add(new(2004,1,new object[]{entity,(byte)0}));
        ev.Instructions.Add(new(2004,15,new object[]{entity,(byte)1}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,request}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,Combat}));
        ev.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
        ev.Instructions.Add(new(2004,1,new object[]{entity,(byte)1}));
        ev.Instructions.Add(new(2004,15,new object[]{entity,(byte)0}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)-1,(byte)0,(byte)0,request}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)-1,(byte)0,(byte)0,Combat}));
        ev.Instructions.Add(new(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)-1}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));
        return ev;
    }
    public static void Verify(EMEVD.Event ev,uint entity) {
        PersonalEvents.Verify(ev);
        var expected=ev.ID==entity?Create(entity):Battle(entity);
        if(ev.ID!=expected.ID||ev.Instructions.Count!=expected.Instructions.Count||
            ev.Instructions.Zip(expected.Instructions).Any(pair=>pair.First.Bank!=pair.Second.Bank||pair.First.ID!=pair.Second.ID||!pair.First.ArgData.SequenceEqual(pair.Second.ArgData)))
            throw new Exception($"Party enemy activation event mismatch: {entity}");
    }
    public static void Test() {
        uint entity=48010000;var ev=Create(entity);
        var enable=ev.Instructions.Single(i=>i.Bank==2004&&i.ID==5&&i.ArgData[4]==1);
        if(enable.ArgData.Length!=8||BitConverter.ToUInt32(enable.ArgData,0)!=entity)throw new Exception("Enable character ABI mismatch");
        int requestAt=ev.Instructions.FindIndex(i=>i.Bank==3&&i.ID==0&&i.ArgData[0]==0&&i.ArgData[1]==1);
        int loadAt=ev.Instructions.FindIndex(i=>i.Bank==2004&&i.ID==29&&i.ArgData[4]==0);
        int readyAt=ev.Instructions.FindIndex(i=>i.Bank==2003&&i.ID==66&&i.ArgData[8]==1);
        if(loadAt<=requestAt||loadAt>=ev.Instructions.IndexOf(enable)||readyAt<=loadAt)throw new Exception("Enemy loading must follow request and precede enable and ready");
        if(!ev.Instructions.Take(requestAt).Any(i=>i.Bank==2004&&i.ID==29&&i.ArgData[4]==1))throw new Exception("Inactive enemies must release backread");
        var waits=ev.Instructions.Where(i=>i.Bank==3&&i.ID==0&&i.ArgData[0]==0).ToArray();
        if(waits.Length!=2||waits[0].ArgData[1]!=1||waits[1].ArgData[1]!=0||waits.Any(i=>BitConverter.ToUInt32(i.ArgData,4)!=45008000))throw new Exception("Activation request gate mismatch");
        var warps=ev.Instructions.Where(i=>i.Bank==2004&&i.ID==42).ToArray();
        if(warps.Length!=12||warps.Any(i=>i.ArgData.Length!=20||i.ArgData[4]!=1||BitConverter.ToUInt32(i.ArgData,16)!=10000))throw new Exception("Formation warp ABI mismatch");
        for(uint slot=0;slot<12;slot++) {
            var conditions=ev.Instructions.Where(i=>i.Bank==3&&i.ID==0&&i.ArgData[0]==slot+1).ToArray();
            if(conditions.Length!=4)throw new Exception("Formation needs four independent flags");
            for(int bit=0;bit<4;bit++)if(conditions[bit].ArgData[1]!=((slot>>bit)&1)||BitConverter.ToUInt32(conditions[bit].ArgData,4)!=Position(entity)+bit)throw new Exception("Formation bit order mismatch");
            if(BitConverter.ToUInt32(warps[slot].ArgData,8)!=Region(entity,slot))throw new Exception("Formation region mismatch");
        }
        var flags=new HashSet<uint>();
        for(uint act=1;act<=3;act++)for(uint slot=0;slot<300;slot++) {
            uint id=48000000+act*10000+slot;
            if(Request(id)/1000!=45008||Ready(id)/1000!=45009||!flags.Add(Request(id))||!flags.Add(Ready(id)))throw new Exception("Activation flag allocation collision");
        }
        Verify(ev,entity);
        var battle=Battle(entity);Verify(battle,entity);
        var gates=battle.Instructions.Where(i=>i.Bank==3&&i.ID==0).ToArray();
        if(gates.Length!=4||gates[0].ArgData[0]!=1||gates[1].ArgData[0]!=1||gates[2].ArgData[0]!=255||gates[3].ArgData[0]!=255)throw new Exception("Combat barrier condition groups mismatch");
        enable.ArgData[4]=0;
        bool rejected=false;try {Verify(ev,entity);}catch(Exception){rejected=true;}
        if(!rejected)throw new Exception("Corrupt activation accepted");
        Console.WriteLine("Verified activation request, enable ABI, stop gate and corrupted event rejection");
    }
    public static void PatchLoading(string input,string output) {
        int total=0;
        for(int act=1;act<=3;act++) {
            string name=$"m45_0{3-act}_00_00.emevd.dcx";
            var file=EMEVD.Read(System.IO.Path.Combine(input,"event",name));
            uint first=(uint)(48000000+act*10000);
            var targets=file.Events.Where(e=>e.ID>=first&&e.ID<first+300).ToArray();
            int expected=new[]{282,242,160}[act-1];
            if(targets.Length!=expected)throw new Exception("Unexpected party enemy event count");
            var untouched=file.Events.Except(targets).ToArray();
            foreach(var ev in targets)file.Events[file.Events.IndexOf(ev)]=Create((uint)ev.ID);
            Directory.CreateDirectory(System.IO.Path.Combine(output,"event"));
            string path=System.IO.Path.Combine(output,"event",name);file.Write(path);var check=EMEVD.Read(path);
            foreach(var ev in targets)Verify(check.Events.Single(e=>e.ID==ev.ID),(uint)ev.ID);
            foreach(var ev in untouched) {
                var actual=check.Events.Single(e=>e.ID==ev.ID);
                if(actual.Instructions.Count!=ev.Instructions.Count||actual.Instructions.Zip(ev.Instructions).Any(p=>p.First.Bank!=p.Second.Bank||p.First.ID!=p.Second.ID||!p.First.ArgData.SequenceEqual(p.Second.ArgData)))throw new Exception("Unrelated event changed");
            }
            total+=targets.Length;
        }
        Console.WriteLine($"Verified {total} enemy loading events; unrelated events retained");
    }
}
