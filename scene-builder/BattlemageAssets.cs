using SoulsFormats;

// 余烬「战斗法师」：属性补正表（AttackElementCorrectParam）里预留 8 行专用行 9670000–9670007（先照抄原版默认行 10000）。
// 运行时把被转换武器原来的补正规则复制进其中一行，再让智力或信仰也计入物理伤害，武器参数改用这一行（native/src/embers.rs 的 sync_battlemage）。
// battlemage-patch：只改 regulation（Linux 上也能运行），其余参数逐字节不变，可重复运行。
static class BattlemageAssets {
    public const int First=9670000,Count=8,Template=10000;
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="AttackElementCorrectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/AttackElementCorrectParam.xml");
        var table=PARAM.Read(file.Bytes);table.ApplyParamdef(def);
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var template=table.Rows.Single(r=>r.ID==Template);
        if(table.Rows.Any(r=>r.ID>=First&&r.ID<First+Count&&!r.Name.StartsWith("Tower battlemage")))throw new Exception("Battlemage element correction range in use");
        int added=0;
        for(int i=0;i<Count;i++) {
            if(table.Rows.Any(r=>r.ID==First+i))continue;
            table.Rows.Add(new PARAM.Row(template){ID=First+i,Name=$"Tower battlemage {i}"});added++;
        }
        table.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));if(added>0)file.Bytes=table.Write();
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows)foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing element correction changed: {row.ID}");
        for(int i=0;i<Count;i++)if(!rows.ContainsKey(First+i))throw new Exception($"Missing battlemage row {First+i}");
        Console.WriteLine($"Battlemage element corrections: {added} added ({First}-{First+Count-1}); other params preserved");
    }
}
