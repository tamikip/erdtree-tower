using SoulsFormats;
// 公开版不再分发 RNG Lands - Ruby 的法术参数：按 retired-spell-rows.txt（表名|行 ID）从 regulation 中剔除这些行
static class RetiredSpellStrip {
    static readonly Dictionary<string,string> Defs=new(){["AtkParam_Pc"]="AtkParam",["Bullet"]="BulletParam",["SpEffectParam"]="SpEffect",["SpEffectVfxParam"]="SpEffectVfx",["Magic"]="MagicParam",["BehaviorParam_PC"]="BehaviorParam"};
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var retired=File.ReadAllLines(Path.Combine(input,"retired-spell-rows.txt")).Select(l=>l.Split('|')).GroupBy(p=>p[0]).ToDictionary(g=>g.Key,g=>g.Select(p=>int.Parse(p[1])).ToHashSet());
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());var removed=0;var counts=new Dictionary<string,int>();
        foreach(var (table,ids) in retired) {
            var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==table);
            var p=PARAM.Read(file.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{Defs.GetValueOrDefault(table,table)}.xml"));
            int n=p.Rows.RemoveAll(r=>ids.Contains(r.ID));
            if(n!=0&&n!=ids.Count)throw new Exception($"Partially present retired rows in {table}: {n}/{ids.Count}");
            counts[table]=p.Rows.Count;removed+=n;if(n>0)file.Bytes=p.Write();
        }
        if(removed==0){Console.WriteLine("Retired spell rows already absent; nothing written");return;}
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files) {
            var table=Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'));
            if(!retired.ContainsKey(table)){if(!f.Bytes.SequenceEqual(before[f.Name]))throw new Exception($"Unrelated param changed: {table}");continue;}
            var def=PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{Defs.GetValueOrDefault(table,table)}.xml");
            var old=PARAM.Read(before[f.Name]);old.ApplyParamdef(def);var now=PARAM.Read(f.Bytes);now.ApplyParamdef(def);
            if(now.Rows.Any(r=>retired[table].Contains(r.ID))||now.Rows.Count!=counts[table])throw new Exception($"Retired rows remain in {table}");
            var rows=now.Rows.ToDictionary(r=>r.ID);
            foreach(var r in old.Rows.Where(r=>!retired[table].Contains(r.ID)))foreach(var c in r.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,rows[r.ID][c.Def.InternalName].Value))throw new Exception($"Kept row changed: {table}:{r.ID}");
        }
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"retired-spell-strip.json"),System.Text.Json.JsonSerializer.Serialize(new{removedRows=removed,sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Removed {removed} retired spell rows from {retired.Count} tables; all other rows and params preserved");
    }
}
