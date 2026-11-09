using SoulsFormats;

// 同品质的重复词条叠加：词条效果是装备的常驻效果（AffixAssets.EffectFirst + 种类×10 + 品质），游戏对同一编号只保留一份，
// 两件装备带同一词条同一品质时只算一次。这里给每个词条效果复制 Copies 份数值相同、编号不同的效果
// （First + 份数×1000 + 原编号偏移），由原生 DLL（native/src/affix_stack.rs）按身上重复的件数挂上。
// 最多 10 件装备（6 个武器栏 + 4 件防具）；第 0 份用于补足原版未激活的收起武器。
static class AffixStackAssets {
    public const int First=9880000,Copies=9,Span=1000;
    static bool Ours(int id)=>id>=First&&id<First+Span*(Copies+1)&&(id-First)%Span<AffixAssets.Kinds*10;
    static bool Affix(int id)=>id>=AffixAssets.EffectFirst&&id<AffixAssets.EffectFirst+AffixAssets.Kinds*10;
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,"SpEffect.xml"));
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        var sources=effects.Rows.Where(r=>Affix(r.ID)).ToList();
        if(sources.Count==0)throw new Exception("Affix effects missing; run affix-patch first");
        if(sources.Any(r=>Convert.ToInt32(r["spCategory"].Value)!=0))throw new Exception("Affix effects must not share a stacking category");
        if(effects.Rows.Any(r=>Ours(r.ID)&&!(r.Name?.StartsWith("Tower affix ")==true&&r.Name.Contains(" stack "))))throw new Exception("Affix copy ID collision");
        var removed=effects.Rows.RemoveAll(r=>Ours(r.ID));
        foreach(var source in sources)for(int copy=0;copy<=Copies;copy++)
            effects.Rows.Add(new PARAM.Row(source){ID=First+copy*Span+source.ID-AffixAssets.EffectFirst,Name=$"{source.Name} stack {copy}"});
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows.Where(r=>!Ours(r.ID)))foreach(var cell in row.Cells)
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        foreach(var source in sources)for(int copy=0;copy<=Copies;copy++) {
            var row=rows[First+copy*Span+source.ID-AffixAssets.EffectFirst];
            foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,source[cell.Def.InternalName].Value))throw new Exception($"Stack copy differs: {row.ID} {cell.Def.InternalName}");
        }
        if(actual.Rows.Count!=original.Rows.Count-removed+sources.Count*(Copies+1))throw new Exception("Unexpected effect count");
        Console.WriteLine($"Affix stacking: {sources.Count} affix effects × {Copies+1} copies ({First}–{First+Span*Copies+AffixAssets.Kinds*10-1}); other params unchanged");
    }
}
