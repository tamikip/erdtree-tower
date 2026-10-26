using SoulsFormats;
using System.Text.Json;

static class MusicAssets {
    const string Assets="outputs/erdtree-tower/scene-assets";
    const uint FlagBase=45020400,EventBase=95050000;
    static string[][] Tracks()=>File.ReadAllLines(Path.Combine(Assets,"bgm-catalog.txt")).Select(l=>l.Split('|')).ToArray();
    static string[] Maps()=>File.ReadAllLines(Path.Combine(Assets,"bgm-maps.txt"));
    static EMEVD.Event TrackEvent(int index,int track) {
        var ev=new EMEVD.Event(EventBase+(uint)index,EMEVD.Event.RestBehaviorType.Restart);PersonalEvents.Local(ev);
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,FlagBase+(uint)index}));
        ev.Instructions.Add(new(2010,10,new object[]{track,0}));
        ev.Instructions.Add(new(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,FlagBase+(uint)index}));
        ev.Instructions.Add(new(2010,10,new object[]{track,-1}));
        ev.Instructions.Add(new(1000,4,new object[]{(byte)1}));return ev;
    }
    static string Signature(EMEVD.Event e)=>JsonSerializer.Serialize(new{e.ID,e.RestBehavior,instructions=e.Instructions.Select(i=>new{i.Bank,i.ID,data=Convert.ToHexString(i.ArgData)}),e.Parameters});
    static bool OwnEvent(long id,int count)=>id>=EventBase&&id<EventBase+count;
    static bool OwnInit(EMEVD.Instruction i,int count)=>i.Bank==2000&&i.ID==0&&i.ArgData.Length>=8&&OwnEvent(BitConverter.ToUInt32(i.ArgData,4),count);
    public static void Patch(string input,string output) {
        var tracks=Tracks();
        if(tracks.Length!=100||tracks.Select(t=>t[1]).Distinct().Count()!=100||tracks.Select(t=>t[2]).Distinct().Count()!=100)throw new Exception("Music catalog must contain 100 unique mappings");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var table=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="WwiseValueToStrParam_BgmBossChrIdConv").Bytes);
        table.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/WwiseValueToStrConvertParamFormat.xml"));
        foreach(var row in tracks)if((string)table.Rows.Single(r=>r.ID==int.Parse(row[1]))["ParamStr"].Value!=row[2])throw new Exception("Soundtrack mapping differs from installed game");
        Directory.CreateDirectory(Path.Combine(output,"event"));
        foreach(var map in Maps()) {
            var path=Path.Combine(input,"event",map+".emevd.dcx");var file=EMEVD.Read(path);
            foreach(var ev in file.Events.Where(e=>OwnEvent(e.ID,tracks.Length))) {
                int index=(int)(ev.ID-EventBase);
                if(Signature(ev)!=Signature(TrackEvent(index,int.Parse(tracks[index][1]))))throw new Exception($"BGM event ID collision: {map}:{ev.ID}");
            }
            foreach(var ev in file.Events.Where(e=>!OwnEvent(e.ID,tracks.Length)))foreach(var ins in ev.Instructions)
                for(int offset=0;offset+4<=ins.ArgData.Length;offset+=4) {
                    uint value=BitConverter.ToUInt32(ins.ArgData,offset);
                    if(value>=FlagBase&&value<FlagBase+tracks.Length)throw new Exception($"BGM flag range already referenced: {map}:{ev.ID}");
                }
            var before=file.Events.Where(e=>!OwnEvent(e.ID,tracks.Length)).ToDictionary(e=>e.ID,e=>{
                if(e.ID!=0)return Signature(e);
                var init=new EMEVD.Event(0);init.Instructions.AddRange(e.Instructions.Where(i=>!OwnInit(i,tracks.Length)));init.Parameters.AddRange(e.Parameters);return Signature(init);
            });
            var init=file.Events.Single(e=>e.ID==0);
            init.Instructions.RemoveAll(i=>OwnInit(i,tracks.Length));
            file.Events.RemoveAll(e=>OwnEvent(e.ID,tracks.Length));
            for(int i=0;i<tracks.Length;i++) {
                file.Events.Add(TrackEvent(i,int.Parse(tracks[i][1])));
                init.Instructions.Add(new(2000,0,new object[]{0,EventBase+(uint)i,0u}));
            }
            var destination=Path.Combine(output,"event",map+".emevd.dcx");file.Write(destination);
            var check=EMEVD.Read(destination);
            foreach(var e in check.Events.Where(e=>!OwnEvent(e.ID,tracks.Length))) {
                EMEVD.Event actual=e;
                if(e.ID==0) {actual=new EMEVD.Event(0);actual.Instructions.AddRange(e.Instructions.Where(i=>!OwnInit(i,tracks.Length)));actual.Parameters.AddRange(e.Parameters);}
                if(!before.TryGetValue(e.ID,out var signature)||signature!=Signature(actual))throw new Exception($"Unrelated event changed during BGM patch: {map}:{e.ID}");
            }
            if(check.Events.Count!=before.Count+tracks.Length)throw new Exception("Music patch event count mismatch");
        }
        foreach(string name in new[]{"bgm-catalog.txt","bgm-maps.txt"}) {
            var source=Path.GetFullPath(Path.Combine(Assets,name));var target=Path.GetFullPath(Path.Combine(output,name));
            if(source!=target)File.Copy(source,target,true);
        }
        File.WriteAllText(Path.Combine(output,"bgm-build.json"),JsonSerializer.Serialize(new{tracks=tracks.Length,maps=Maps().Length,flagBase=FlagBase,eventBase=EventBase,pools=tracks.GroupBy(t=>t[0]).ToDictionary(g=>g.Key,g=>g.Count()),unrelatedEventsPreserved=true,regulationUnchanged=true},new JsonSerializerOptions{WriteIndented=true}));
        Verify(output);
    }
    public static void Verify(string directory) {
        var tracks=Tracks();
        foreach(var map in Maps()) {
            var file=EMEVD.Read(Path.Combine(directory,"event",map+".emevd.dcx"));
            var init=file.Events.Single(e=>e.ID==0);
            for(int i=0;i<tracks.Length;i++) {
                var id=EventBase+(uint)i;var ev=file.Events.Single(e=>e.ID==id);PersonalEvents.Verify(ev);
                if(Signature(ev)!=Signature(TrackEvent(i,int.Parse(tracks[i][1]))))throw new Exception($"Music lifecycle mismatch: {map}:{id}");
                if(init.Instructions.Count(ins=>OwnInit(ins,tracks.Length)&&BitConverter.ToUInt32(ins.ArgData,4)==id)!=1)throw new Exception($"Music constructor missing or duplicated: {map}:{id}");
            }
        }
        Console.WriteLine($"Verified {tracks.Length} music mappings across {Maps().Length} maps; local playback, flag gates and start/stop lifecycle");
    }
}
