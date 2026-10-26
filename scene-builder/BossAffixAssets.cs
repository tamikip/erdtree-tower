using SoulsFormats;
static class BossAffixAssets {
    public const int First=9620000;
    const int Resident=7030,Neutral=90400,PlayerStatus=4051;
    static readonly string[] Cuts=["slashDamageCutRate","blowDamageCutRate","thrustDamageCutRate","neutralDamageCutRate","magicDamageCutRate","fireDamageCutRate","thunderDamageCutRate","darkDamageCutRate"];
    static readonly string[] Attacks=["physicsAttackPowerRate","magicAttackPowerRate","fireAttackPowerRate","thunderAttackPowerRate","darkAttackPowerRate"];
    static readonly string[] Kept=["spCategory","categoryPriority","effectTargetOpposeTarget","effectTargetFriendlyTarget","effectTargetSelfTarget","effectTargetSelf","effectTargetFriend","effectTargetEnemy","effectTargetPlayer","effectTargetAI","effectTargetLive","effectTargetGhost"];
    static (string,object)[] All(string[] fields,float value)=>fields.Select(f=>(f,(object)value)).ToArray();
    static (int id,string name,bool player,(string,object)[] fields)[] Rows=>[
        (First,"Tower boss affix stalwart",false,[("toughnessDamageCutRate",0.5f),("vfxId",8570)]),
        (First+10,"Tower boss affix bulwark",false,[..All(Cuts,0.75f),("vfxId",8510)]),
        (First+20,"Tower boss affix frenzied",false,[..All(Attacks,1.25f),..All(Cuts,1.1f),("vfxId",1605000)]),
        (First+30,"Tower boss affix regenerating",false,[("motionInterval",1f),("changeHpRate",-0.3f),("vfxId",1643000)]),
        (First+40,"Tower boss affix rot aura",false,[("vfxId",5)]),
        (First+41,"Tower boss affix rot buildup",true,[("diseaseAttackPower",20)]),
        (First+70,"Tower boss affix envious rage",false,[("effectEndurance",10f),..All(Attacks,1.3f),("vfxId",1605000)]),
        (First+80,"Tower boss affix proud",false,[..All(Attacks,1.2f),("vfxId",1605000)]),
        (First+90,"Tower boss affix golden shield",false,[("effectEndurance",5f),..All(Cuts,0.3f),("vfxId",1660000)]),
        (First+95,"Tower boss affix reborn",false,[("effectEndurance",3f),("vfxId",1643000)]),
    ];
    static PARAM.Row Create(PARAM effects,(int id,string name,bool player,(string,object)[] fields) spec) {
        PARAM.Row row;
        if(spec.player) {
            row=new PARAM.Row(effects.Rows.Single(r=>r.ID==PlayerStatus)){ID=spec.id,Name=spec.name};
            foreach(var (key,value) in new (string,object)[]{("effectEndurance",0f),("motionInterval",0f),("changeHpRate",0f),("changeHpPoint",0),("diseaseAttackPower",0),("cycleOccurrenceSpEffectId",-1),("vfxId",-1)})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
        } else {
            var neutral=effects.Rows.Single(r=>r.ID==Neutral);
            row=new PARAM.Row(effects.Rows.Single(r=>r.ID==Resident)){ID=spec.id,Name=spec.name};
            foreach(var cell in row.Cells.Where(c=>!Kept.Contains(c.Def.InternalName)))cell.Value=neutral[cell.Def.InternalName].Value;
            foreach(var (key,value) in new (string,object)[]{("effectEndurance",-1f),("spCategory",0),("categoryPriority",0),("saveCategory",-1)})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
        }
        foreach(var (key,value) in spec.fields)row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
        return row;
    }
    static void Verify(PARAM effects) {
        foreach(var spec in Rows) {
            var row=effects.Rows.Single(r=>r.ID==spec.id);var expected=Create(effects,spec);
            foreach(var cell in expected.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,row[cell.Def.InternalName].Value))throw new Exception($"Boss affix effect mismatch: {spec.id} {cell.Def.InternalName}");
            if(!spec.player&&Convert.ToInt32(row["spCategory"].Value)!=0)throw new Exception($"Boss affix effect must not share a stacking category: {spec.id}");
        }
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        if(effects.Rows.Any(r=>r.ID>=First&&r.ID<First+100))throw new Exception("Boss affix effect range 9620000-9620099 already in use");
        foreach(var spec in Rows)effects.Rows.Add(Create(effects,spec));
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);Verify(actual);
        if(actual.Rows.Count!=original.Rows.Count+Rows.Length)throw new Exception("Unexpected effect count");
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows)foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"boss-affix-build.json"),System.Text.Json.JsonSerializer.Serialize(new{effects=Rows.Select(r=>r.id),sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified {Rows.Length} boss affix effects; all existing regulation data preserved");
    }
}
