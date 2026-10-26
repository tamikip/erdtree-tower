using SoulsFormats;
static class ShopOrderAssets {
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="ShopLineupParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/ShopLineupParam.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var shop=PARAM.Read(file.Bytes);shop.ApplyParamdef(def);
        var before=shop.Rows.Select(r=>r.ID).ToList();
        shop.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        if(before.SequenceEqual(shop.Rows.Select(r=>r.ID))){Console.WriteLine("ShopLineupParam already sorted; nothing written");return;}
        if(shop.Rows.Select(r=>r.ID).Distinct().Count()!=shop.Rows.Count)throw new Exception("Duplicate ShopLineupParam IDs");
        file.Bytes=shop.Write();
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        if(!actual.Rows.Select(r=>r.ID).SequenceEqual(before.OrderBy(id=>id)))throw new Exception("ShopLineupParam order not normalized");
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows)foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Shop row changed: {row.ID}");
        string Hash(string file)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(file)));
        File.WriteAllText(Path.Combine(output,"shop-order-build.json"),System.Text.Json.JsonSerializer.Serialize(new{rows=actual.Rows.Count,sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Sorted {actual.Rows.Count} ShopLineupParam rows; row contents and all other params preserved");
    }
}
