using SoulsFormats;

static class ContractAssets {
    public const int First=9600000;
    static void Set(PARAM.Row r,string key,object value)=>r[key].Value=Convert.ChangeType(value,r[key].Value.GetType());
    static void Damage(PARAM.Row r,float rate) {
        foreach(var element in new[]{"physics","magic","fire","thunder","dark"})Set(r,element+"AttackRate",rate);
        foreach(var target in new[]{"Player","Enemy"})foreach(var element in new[]{"Physics","Magic","Fire","Thunder","Dark"})Set(r,$"atk{target}DmgCorrectRate_{element}",1f);
    }
    public static void Build(PARAM p,Action<string,PARAM> save) {
        PARAM.Row Add(int offset,int source) {
            int id=First+offset;
            if(p.Rows.Any(r=>r.ID==id))throw new Exception("Contract effect ID conflict");
            var r=new PARAM.Row(p.Rows.Single(r=>r.ID==source)){ID=id,Name="Tower act contract"};
            Set(r,"iconId",-1);Set(r,"effectEndurance",-1f);Set(r,"spCategory",0);Set(r,"categoryPriority",0);
            Set(r,"saveCategory",-1);Set(r,"dontDeleteOnDead",0);p.Rows.Add(r);return r;
        }
        PARAM.Row Plain(int offset) {
            var r=Add(offset,321300);Damage(r,1f);Set(r,"magicSubCategoryChange1",0);return r;
        }
        Damage(Add(0,321300),1.2f);
        Damage(Add(1,312310),0.8f);
        Damage(Add(10,321800),1.25f);
        Set(Plain(11),"artsConsumptionRate",1.25f);
        Damage(Add(20,321200),1.3f);
        var vulnerable=Plain(21);
        foreach(var element in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(vulnerable,element+"DamageCutRate",1.1f);
        Damage(Add(30,322000),1.3f);
        Set(Plain(31),"consumeStaminaRate",1.2f);
        Damage(Add(40,330010),1.2f);Damage(Add(41,330500),1.2f);
        var mana=Plain(42);Set(mana,"magicConsumptionRate",1.2f);Set(mana,"miracleConsumptionRate",1.2f);
        Damage(Add(50,321500),1.3f);Damage(Add(51,312310),0.8f);
        AddExpanded(p);
        save("SpEffectParam",p);
    }
    public static void Verify(PARAM p,PARAM weapons,PARAM goods,PARAM reinforce) {
        foreach(var id in new[]{2000000,3000000,3180000,11000000,14000000,4000000,4040000,12060000,7140000,9000000,10000000,22000000,31130000,16000000,34000000,34010000,34040000,33000000,33130000,33200000,40000000,40020000,41000000,43020000}) {
            var baseRow=weapons.Rows.Single(r=>r.ID==id);
            var cap=Convert.ToInt32(baseRow["originEquipWep16"].Value)<0?10:25;
            int chain=Convert.ToInt32(baseRow["reinforceTypeId"].Value);
            for(int level=0;level<=cap;level++)if(!reinforce.Rows.Any(r=>r.ID==chain+level))throw new Exception($"Missing contract gift reinforcement {id}+{level}");
        }
        foreach(var id in new[]{50000000,53000000})if(!weapons.Rows.Any(r=>r.ID==id))throw new Exception("Missing gift ammunition");
        foreach(var id in new[]{4000,6000})if(!goods.Rows.Any(r=>r.ID==id))throw new Exception("Missing gift spell");
        foreach(var (offset,source,rate) in new[]{(0,321300,1.2f),(1,312310,0.8f),(10,321800,1.25f),(20,321200,1.3f),(30,322000,1.3f),(40,330010,1.2f),(41,330500,1.2f),(50,321500,1.3f),(51,312310,0.8f)}) {
            var r=p.Rows.Single(r=>r.ID==First+offset);var original=p.Rows.Single(r=>r.ID==source);
            foreach(var key in new[]{"magicSubCategoryChange1","magicSubCategoryChange2","magicSubCategoryChange3","stateInfo","magParamChange","miracleParamChange"})
                if(!Equals(r[key].Value,original[key].Value))throw new Exception("Contract lost native attack filtering");
            if(Convert.ToSingle(r["physicsAttackRate"].Value)!=rate||Convert.ToInt32(r["saveCategory"].Value)!=-1)throw new Exception("Contract multiplier/lifetime mismatch");
        }
        foreach(var (offset,key,value) in new[]{(11,"artsConsumptionRate",1.25f),(21,"neutralDamageCutRate",1.1f),(31,"consumeStaminaRate",1.2f),(42,"magicConsumptionRate",1.2f),(42,"miracleConsumptionRate",1.2f)})
            if(Convert.ToSingle(p.Rows.Single(r=>r.ID==First+offset)[key].Value)!=value)throw new Exception("Contract drawback mismatch");
        VerifyExpanded(p);
        Console.WriteLine("Verified ten contract effects, native attack filters and drawbacks");
    }
    static PARAM.Row Expanded(PARAM p,int offset) {
        var r=new PARAM.Row(p.Rows.Single(r=>r.ID==(offset==60?320900:First+11))){ID=First+offset,Name="Tower expanded act contract"};
        Set(r,"iconId",-1);Set(r,"effectEndurance",-1f);Set(r,"spCategory",0);Set(r,"categoryPriority",0);
        Set(r,"saveCategory",-1);Set(r,"dontDeleteOnDead",0);Set(r,"artsConsumptionRate",1f);
        switch(offset) {
            case 60: Damage(r,1.3f/0.9f);break;
            case 61: Damage(r,0.9f);break;
            case 70: Damage(r,1.15f);Set(r,"staminaRecoverChangeSpeed",5);break;
            case 71: foreach(var kind in new[]{"slash","blow","thrust","neutral"})Set(r,kind+"DamageCutRate",1.1f);break;
            case 80: Damage(r,1.25f);break;
            case 81: Set(r,"changeHpEstusFlaskCorrectRate",0.8f);break;
            case 90: Set(r,"changeHpEstusFlaskCorrectRate",0.65f);Set(r,"changeMpEstusFlaskCorrectRate",0.65f);break;
            default:throw new Exception("Unknown contract effect");
        }
        return r;
    }
    static readonly int[] ExpandedOffsets=[60,61,70,71,80,81,90];
    static void AddExpanded(PARAM p) {
        foreach(int offset in ExpandedOffsets) {
            if(p.Rows.Any(r=>r.ID==First+offset))throw new Exception("Expanded contract ID conflict");
            p.Rows.Add(Expanded(p,offset));
        }
    }
    static void VerifyExpanded(PARAM p) {
        foreach(int offset in ExpandedOffsets) {
            var expected=Expanded(p,offset);var actual=p.Rows.Single(r=>r.ID==First+offset);
            foreach(var cell in expected.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,actual[cell.Def.InternalName].Value))throw new Exception($"Expanded contract mismatch: {offset} {cell.Def.InternalName}");
        }
        var critical=p.Rows.Single(r=>r.ID==First+60);
        if(Convert.ToInt32(critical["stateInfo"].Value)!=367||Math.Abs(Convert.ToSingle(critical["physicsAttackRate"].Value)*0.9f-1.3f)>0.00001f)throw new Exception("Execution critical filtering/rate mismatch");
    }
    public static void PatchExpanded(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);AddExpanded(effects);
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);VerifyExpanded(actual);
        if(actual.Rows.Count!=original.Rows.Count+ExpandedOffsets.Length)throw new Exception("Unexpected effect count");
        foreach(var row in original.Rows) {
            var match=actual.Rows.Single(r=>r.ID==row.ID);
            foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,match[cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        }
        Console.WriteLine("Verified seven new effect rows: critical filter, conditional boons, flask multipliers; all existing regulation data preserved");
    }
}
