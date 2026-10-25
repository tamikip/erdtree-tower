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
    public static void Audit(string directory) {
        int count=0;
        foreach(var path in Directory.EnumerateFiles(Path.Combine(directory,"map/mapstudio"),"*.msb.dcx")) {
            if(Path.GetFileName(path).StartsWith("m11_10"))continue;
            foreach(var part in MSBE.Read(path).Parts.Enemies.Where(p=>p.Name.StartsWith("tower_service_")||p.Name.StartsWith("tower_camp_service_")||p.Name.StartsWith("tower_party_"))) {Verify(part);count++;}
        }
        int expected=684+3+File.ReadAllLines("outputs/erdtree-tower/scene-assets/party-services.txt").Length;
        if(count!=expected)throw new Exception($"Expected {expected} cross-map characters, found {count}");
        Console.WriteLine($"Verified visibility masks for {count} cross-map characters");
    }
}
