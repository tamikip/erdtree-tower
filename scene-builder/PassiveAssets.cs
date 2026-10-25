using SoulsFormats;
static class PassiveAssets {
    const int First=9800000;
    static readonly string[] Resistances=["Poison","Disease","Blood","Freeze","Sleep","Madness","Curse"];
    static readonly string[] Elements=["magic","fire","thunder","dark"];
    static readonly string[] Physical=["slash","blow","thrust","neutral"];
    static readonly string[] Names=["Prophet resistance","Confessor elemental defense","Hero physical defense","Bandit runes","Samurai stamina recovery"];
    static void Set(PARAM.Row row,string key,object value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
    static Dictionary<string,object> Values(int i) {
        var values=new Dictionary<string,object>{{"artsConsumptionRate",1f}};
        if(i==0)foreach(var kind in Resistances)values[$"change{kind}ResistPoint"]=50;
        if(i==1)foreach(var element in Elements)values[element+"DamageCutRate"]=0.92f;
        if(i==2)foreach(var kind in Physical)values[kind+"DamageCutRate"]=0.95f;
        if(i==3)values["soulRate"]=1.1f;
        if(i==4)values["staminaRecoverChangeSpeed"]=5;
        return values;
    }
    static PARAM.Row Create(PARAM effects,int i) {
        var row=new PARAM.Row(effects.Rows.Single(r=>r.ID==9600011)){ID=First+i,Name="Tower "+Names[i]};
        foreach(var (key,value) in Values(i))Set(row,key,value);
        return row;
    }
    public static void Build(PARAM effects,Action<string,PARAM> save) {
        for(int i=0;i<5;i++) {
            if(effects.Rows.Any(r=>r.ID==First+i))throw new Exception("Class passive effect collision");
            effects.Rows.Add(Create(effects,i));
        }
        save("SpEffectParam",effects);
    }
    public static void Verify(PARAM effects,PARAM goods) {
        for(int i=0;i<5;i++) {
            var row=effects.Rows.Single(r=>r.ID==First+i);
            var source=effects.Rows.Single(r=>r.ID==9600011);
            var expected=Values(i);var changed=expected.Keys.ToHashSet();
            foreach(var (key,value) in expected)if(!Equals(row[key].Value,Convert.ChangeType(value,row[key].Value.GetType())))throw new Exception($"Passive mismatch: {Names[i]} {key}");
            if(Convert.ToSingle(row["artsConsumptionRate"].Value)!=1f)throw new Exception("Unwanted passive skill cost");
            foreach(var cell in source.Cells.Where(c=>!changed.Contains(c.Def.InternalName)))if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,row[cell.Def.InternalName].Value))throw new Exception("Unrelated passive effect change");
        }
        if(Convert.ToInt32(goods.Rows.Single(r=>r.ID==8185)["maxNum"].Value)<5)throw new Exception("Larval Tear stack below Wretch gift");
        if(!goods.Rows.Any(r=>r.ID==10020))throw new Exception("Missing Vagabond Sacred Tear");
        Console.WriteLine("Verified five class effects, unchanged template fields, and starting gift capacity");
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Patch requires a separate output directory");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="SpEffectParam");
        PARAM Read(string name,string def) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{def}.xml"));return p;
        }
        var effects=Read("SpEffectParam","SpEffect");var goods=Read("EquipParamGoods","EquipParamGoods");
        var original=Read("SpEffectParam","SpEffect");
        for(int i=2;i<5;i++) {
            if(effects.Rows.Any(r=>r.ID==First+i))throw new Exception("Class passive effect collision");
            effects.Rows.Add(Create(effects,i));
        }
        Verify(effects,goods);
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();
        Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(effects.AppliedParamdef);
        Verify(actual,goods);
        if(actual.Rows.Count!=original.Rows.Count+3)throw new Exception("Unexpected effect row count");
        foreach(var row in original.Rows) {
            var match=actual.Rows.Single(r=>r.ID==row.ID);
            foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,match[cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        }
        Console.WriteLine("Added only three effects; all existing rows and other regulation entries preserved");
    }
    public static void FixBandit(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        var bandit=effects.Rows.Single(r=>r.ID==First+3);
        Console.WriteLine($"Bandit soulRate: {bandit["soulRate"].Value} -> 1.1");
        Set(bandit,"soulRate",1.1f);file.Bytes=effects.Write();
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        if(actual.Rows.Count!=original.Rows.Count)throw new Exception("Unexpected row count");
        foreach(var row in original.Rows) {
            var after=actual.Rows.Single(r=>r.ID==row.ID);
            foreach(var cell in row.Cells) {
                var expected=row.ID==First+3&&cell.Def.InternalName=="soulRate"?1.1f:cell.Value;
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(expected,after[cell.Def.InternalName].Value))throw new Exception($"Unexpected change: {row.ID} {cell.Def.InternalName}");
            }
        }
        var multiplier=Convert.ToSingle(actual.Rows.Single(r=>r.ID==First+3)["soulRate"].Value);
        if(Math.Abs(1000*multiplier-1100)>0.001f)throw new Exception("Bandit must grant 110%, not 10%");
        Console.WriteLine("Verified Bandit +10% reward multiplier; only one parameter cell changed");
    }
}
