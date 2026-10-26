using SoulsFormats;
static class ScalingCorrectionFix {
    public const int Template=10000,First=9860001,Last=9860031;
    // 新建的补正行除 is*Correct_by* 开关外必须与原版一致：overwrite*=-1（不覆盖补正率），Influence*=100（属性影响率 100%）
    static bool Rate(string field)=>field.StartsWith("overwrite")||field.StartsWith("Influence");
    public static bool Neutral(PARAM.Row row,PARAM.Row template)=>row.Cells.Where(c=>Rate(c.Def.InternalName)).All(c=>Equals(c.Value,template[c.Def.InternalName].Value));
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="AttackElementCorrectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/AttackElementCorrectParam.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var param=PARAM.Read(file.Bytes);param.ApplyParamdef(def);
        var template=param.Rows.Single(r=>r.ID==Template);var rows=param.Rows.Where(r=>r.ID>=First&&r.ID<=Last).ToList();
        if(rows.Count!=Last-First+1)throw new Exception("Expected 31 tower scaling correction rows");
        if(rows.All(r=>Neutral(r,template))){Console.WriteLine("Scaling correction rates already neutral; nothing written");return;}
        foreach(var row in rows)foreach(var cell in row.Cells.Where(c=>Rate(c.Def.InternalName)))cell.Value=template[cell.Def.InternalName].Value;
        file.Bytes=param.Write();
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        var fixedRows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows)foreach(var cell in row.Cells) {
            var name=cell.Def.InternalName;bool ours=row.ID>=First&&row.ID<=Last&&Rate(name);
            var expected=ours?template[name].Value:cell.Value;
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(expected,fixedRows[row.ID][name].Value))throw new Exception($"Unexpected correction change: {row.ID} {name}");
        }
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"scaling-correction-build.json"),System.Text.Json.JsonSerializer.Serialize(new{rows=rows.Count,template=Template,sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Restored vanilla overwrite/influence rates on {rows.Count} scaling correction rows; scaling switches and all other params preserved");
    }
}
