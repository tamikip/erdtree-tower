using SoulsFormats;

static class PartyModels {
    public static void Prepare(MSBE map) {
        foreach(var model in map.Models.GetEntries().Where(m=>m is MSBE.Model.Enemy or MSBE.Model.Player))
            if(string.IsNullOrWhiteSpace(model.SibPath))model.SibPath=$@"N:\GR\data\Model\chr\{model.Name}\sib\{model.Name}.sib";
    }
    public static void Verify(MSBE map) {
        foreach(var model in map.Models.GetEntries().Where(m=>m is MSBE.Model.Enemy or MSBE.Model.Player))
            if(!model.SibPath.Replace('\\','/').EndsWith($"/{model.Name}.sib",StringComparison.OrdinalIgnoreCase))
                throw new Exception($"Character model resource path missing or mismatched: {model.Name}");
    }
    public static void Process(string input,string output,bool patch) {
        int count=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(input,"map/mapstudio"),"*.msb.dcx")) {
            var map=MSBE.Read(path);
            if(!map.Parts.Enemies.Any(p=>p.Name.StartsWith("tower_")))continue;
            if(patch)Prepare(map);
            Verify(map);
            if(patch) {var dest=Path.Combine(output,"map/mapstudio",Path.GetFileName(path));Directory.CreateDirectory(Path.GetDirectoryName(dest)!);map.Write(dest);Verify(MSBE.Read(dest));}
            count++;
        }
        if(count==0)throw new Exception("No tower character maps inspected");
        Console.WriteLine($"Verified model resource paths in {count} maps");
    }
}
