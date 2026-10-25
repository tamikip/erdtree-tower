using SoulsFormats;

static class ArcherAssets {
    public static void Normalize(MSBE.Part.Enemy part) {
        if(part.ModelName!="c3170")return;
        for(int i=0;i<part.SpEffectSetParamID.Length;i++)if(part.SpEffectSetParamID[i]==106100)part.SpEffectSetParamID[i]=0;
    }
    public static void Verify(string input) {
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_01_00_00.msb.dcx"));
        var archers=map.Parts.Enemies.Where(p=>p.ModelName=="c3170"&&p.NPCParamID>=900000000).ToArray();
        if(archers.Length!=12||archers.Any(p=>p.SpEffectSetParamID.Contains(106100)))throw new Exception("Tower archers retain world apparition effects or entity count changed");
        Console.WriteLine("Verified 12 tower archers: no inherited apparition effects");
    }
    public static void Patch(string input,string output) {
        const string file="map/mapstudio/m45_01_00_00.msb.dcx";
        var map=MSBE.Read(Path.Combine(input,file));var original=map.Write();
        var changed=map.Parts.Enemies.Where(p=>p.ModelName=="c3170"&&p.NPCParamID>=900000000)
            .ToDictionary(p=>p.EntityID,p=>(int[])p.SpEffectSetParamID.Clone());
        foreach(var p in map.Parts.Enemies.Where(p=>changed.ContainsKey(p.EntityID)))Normalize(p);
        var path=Path.Combine(output,file);Directory.CreateDirectory(Path.GetDirectoryName(path)!);map.Write(path);
        Verify(output);
        var check=MSBE.Read(path);
        foreach(var p in check.Parts.Enemies.Where(p=>changed.ContainsKey(p.EntityID)))p.SpEffectSetParamID=changed[p.EntityID];
        if(!check.Write().SequenceEqual(original))throw new Exception("Unrelated map data changed");
        Console.WriteLine("Roundtrip verified: only archer effect sets changed; all other map data preserved");
    }
}
