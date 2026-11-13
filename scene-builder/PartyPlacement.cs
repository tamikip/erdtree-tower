using SoulsFormats;

static class PartyPlacement {
    public static void Prepare(MSBE.Part.Enemy part) {
        part.MapStudioLayer=uint.MaxValue;
        part.Unk1.DisplayGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();
        part.Unk1.DrawGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();
    }
    public static void Verify(MSBE.Part.Enemy part) {
        if(part.MapStudioLayer!=uint.MaxValue||part.Unk1.DisplayGroups.Any(v=>v!=uint.MaxValue)||part.Unk1.DrawGroups.Any(v=>v!=uint.MaxValue))
            throw new Exception($"Cross-map character retains source visibility restrictions: {part.EntityID}");
    }
    public static void Audit(string directory,bool trainingOnly=false) {
        int count=0,combat=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(directory,"map/mapstudio"),"*.msb.dcx")) {
            if(trainingOnly&&!new[]{"m45_00_00_00.msb.dcx","m45_01_00_00.msb.dcx","m45_02_00_00.msb.dcx"}.Contains(Path.GetFileName(path)))continue;
            if(Path.GetFileName(path).StartsWith("m11_10"))continue;
            foreach(var part in MSBE.Read(path).Parts.Enemies.Where(p=>p.Name.StartsWith("tower_service_")||p.Name.StartsWith("tower_camp_service_")||p.Name.StartsWith("tower_party_"))) {Verify(part);count++;if(part.Name.StartsWith("tower_party_"))combat++;}
        }
        int expected=684;
        var dlc=Path.Combine(directory,"party-dlc-slots.tsv");if(File.Exists(dlc))expected+=File.ReadAllLines(dlc).Length;
        var extra=Path.Combine(directory,"party-extra-slots.tsv");if(File.Exists(extra))expected+=File.ReadAllLines(extra).Length;
        if(combat!=expected)throw new Exception($"Expected {expected} combat characters, found {combat}");
        Console.WriteLine($"Verified visibility masks for {count} cross-map characters ({combat} combat, {count-combat} baked services)");
    }
}
