using SoulsFormats;
using System.Numerics;

static class PartyHubAssets {
    static readonly uint[] Sources=[11100710,11100730];
    static readonly Vector3[] Positions=[new(-329.5f,-21.9f,-309),new(-329.5f,-21.9f,-302)];
    public static void Build(string input,string output) {
        const string name="m11_10_00_00.msb.dcx";
        var map=MSBE.Read(Path.Combine(input,name));
        var originalCount=map.Parts.Enemies.Count;
        var anchor=map.Parts.Enemies.Single(e=>e.EntityID==Sources[1]);
        for(int i=0;i<Sources.Length;i++) {
            uint id=45001800u+(uint)i;
            if(map.Parts.GetEntries().Any(p=>p.EntityID==id))throw new Exception($"Party hub entity collision: {id}");
            var part=(MSBE.Part.Enemy)map.Parts.Enemies.Single(e=>e.EntityID==Sources[i]).DeepCopy();
            part.Name=$"tower_hub_service_{i}";
            part.InstanceID=9800+i;
            if(map.Parts.Enemies.Any(e=>e.ModelName==part.ModelName&&e.InstanceID==part.InstanceID))throw new Exception("Party hub instance collision");
            part.EntityID=id;part.EntityGroupIDs=new uint[8];part.TalkID=-1;
            part.Position=Positions[i];part.Rotation=new Vector3(0,-90,0);
            part.CollisionPartName=anchor.CollisionPartName;part.WalkRouteName=null;
            part.BackupEventAnimID=-1;part.ChrActivateCondParamID=0;
            map.Parts.Enemies.Add(part);
        }
        var directory=Path.Combine(output,"map/mapstudio");Directory.CreateDirectory(directory);
        var path=Path.Combine(directory,name);map.Write(path);
        var result=MSBE.Read(path);
        if(result.Parts.Enemies.Count!=originalCount+2)throw new Exception("Party hub entity count mismatch");
        for(int i=0;i<Sources.Length;i++) {
            var part=result.Parts.Enemies.Single(e=>e.EntityID==45001800u+i);
            if(part.Position!=Positions[i]||Math.Abs(part.Rotation.Y+90)>0.001f||part.TalkID!=-1||part.EntityGroupIDs.Any(id=>id!=0)
                ||!result.Parts.Collisions.Any(c=>c.Name==part.CollisionPartName)
                ||!result.Models.GetEntries().Any(m=>m.Name==part.ModelName))throw new Exception($"Party hub roundtrip validation failed: id={part.EntityID} pos={part.Position} rot={part.Rotation} talk={part.TalkID} groups={string.Join(',',part.EntityGroupIDs)} collision={part.CollisionPartName} model={part.ModelName}");
        }
        Console.WriteLine($"Verified staged hub: {originalCount} original NPCs retained, 2 map-native service entities added: {path}");
    }
    public static void PatchStanding(string input,string output) {
        const string name="m11_10_00_00.msb.dcx";
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio",name));
        var original=MSBE.Read("work/scene-cache/"+name);
        int index=map.Parts.Enemies.FindIndex(p=>p.EntityID==45001800);
        if(index<0)throw new Exception("Hub guide missing");
        var previous=map.Parts.Enemies[index];
        var part=(MSBE.Part.Enemy)original.Parts.Enemies.Single(e=>e.EntityID==11100710).DeepCopy();
        part.Name=previous.Name;part.EntityID=previous.EntityID;part.InstanceID=previous.InstanceID;
        part.Position=previous.Position;part.Rotation=previous.Rotation;part.CollisionPartName=previous.CollisionPartName;
        part.EntityGroupIDs=new uint[8];part.TalkID=-1;part.WalkRouteName=null;part.BackupEventAnimID=-1;part.ChrActivateCondParamID=0;
        PartyPlacement.Prepare(part);map.Parts.Enemies[index]=part;PartyModels.Prepare(map);PartyModels.Verify(map);
        var destination=Path.Combine(output,"map/mapstudio",name);Directory.CreateDirectory(Path.GetDirectoryName(destination)!);map.Write(destination);
        var check=MSBE.Read(destination);var guide=check.Parts.Enemies.Single(p=>p.EntityID==45001800);
        if(guide.ModelName!="c0000"||guide.NPCParamID!=523200079||guide.ThinkParamID!=523200000||guide.CharaInitID!=23200||guide.Position!=previous.Position||guide.TalkID!=-1||check.Parts.Enemies.Count!=map.Parts.Enemies.Count)throw new Exception("Standing hub guide mismatch");
        PartyPlacement.Verify(guide);Console.WriteLine("Standing hub guide verified; service identity and other NPCs retained");
    }
}
