using SoulsFormats;
// 职业重做后的职业机制：六个职业的效果放在 9810000 段（编号 = 9810000 + 职业序号×10 + 序号）。
// 条件效果复制原版护符的判定链（珍珠盾护符的举盾判定、介错刀的致命一击触发），常驻数值复制 9600011 或词条系统用过的护符行。运行时只挂常驻的那几条。
static class ClassAssets {
    public const int First=9810000;
    static readonly string[] AllCuts=["slash","blow","thrust","neutral","magic","fire","thunder","dark"];
    static (string,object)[] Cuts(float rate)=>AllCuts.Select(k=>(k+"DamageCutRate",(object)rate)).ToArray();
    static (string,object)[] VersusEnemies(float rate)=>new[]{"Physics","Magic","Fire","Thunder","Dark"}.Select(e=>($"atkEnemyDmgCorrectRate_{e}",(object)rate)).ToArray();
    // (编号, 名字, 复制的原版行, 攻击倍率（0 为不改）, 保留原图标, 其他字段)
    // 守誓骑士沿用珍珠盾的举盾判定；图标放在常驻行，避免 0.1 秒的触发行反复闪烁。
    // 影刃复制介错刀的三段触发（常驻监视致命一击 → 触发 → 增伤），编号保持相邻
    static (int id,string name,int template,float attack,bool icon,(string,object)[] fields)[] Rows=>[
        (First+0,"Tower class oathsworn guard watch",20380900,0f,true,[("cycleOccurrenceSpEffectId",First+1)]),
        (First+1,"Tower class oathsworn guarding",20380901,0f,false,[..Cuts(0.75f),("cycleOccurrenceSpEffectId",First+2)]),
        (First+2,"Tower class oathsworn guarding icon",20380902,0f,false,[("effectEndurance",0.1f)]),
        (First+3,"Tower class oathsworn guard stamina",341000,1f,false,[("guardStaminaMult",0.7f)]),
        (First+10,"Tower class berserker low health guard",9600011,0f,false,[..Cuts(0.85f),("artsConsumptionRate",1f)]),
        (First+11,"Tower class berserker poise",9600011,0f,false,[("toughnessDamageCutRate",0.7f),("artsConsumptionRate",1f)]),
        (First+20,"Tower class shadow critical watch",20382300,0f,false,[]),
        (First+21,"Tower class shadow critical trigger",20382301,0f,false,[("cycleOccurrenceSpEffectId",First+22)]),
        (First+22,"Tower class shadow after critical",20382302,0f,true,[("effectEndurance",10f),..VersusEnemies(1.2f)]),
        (First+23,"Tower class shadow stamina",9600011,0f,false,[("staminaRecoverChangeSpeed",5),("artsConsumptionRate",1f)]),
        // 星辉术士偏爆发：魔力伤害 +15%（魔法消耗不再降低）；祷誓者偏续航：祷告消耗 -15%，专注回复与胜利回血由 DLL 处理
        (First+30,"Tower class sorcerer starsurge",9600011,0f,false,[("magicAttackRate",1.15f),("artsConsumptionRate",1f)]),
        (First+40,"Tower class cleric focus",9600011,0f,false,[("miracleConsumptionRate",0.85f),("artsConsumptionRate",1f)]),
    ];
    static void Set(PARAM.Row r,string key,object value)=>r[key].Value=Convert.ChangeType(value,r[key].Value.GetType());
    static PARAM.Row Create(PARAM effects,(int id,string name,int template,float attack,bool icon,(string,object)[] fields) spec) {
        var row=new PARAM.Row(effects.Rows.Single(r=>r.ID==spec.template)){ID=spec.id,Name=spec.name};
        if(!spec.icon)Set(row,"iconId",-1);
        // 不与玩家自己装备的原版护符共用类别，两者可以同时生效
        foreach(var (key,value) in new (string,object)[]{("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0)})Set(row,key,value);
        if(spec.attack>0) {
            foreach(var element in new[]{"physics","magic","fire","thunder","dark"})Set(row,element+"AttackRate",spec.attack);
            foreach(var target in new[]{"Player","Enemy"})foreach(var element in new[]{"Physics","Magic","Fire","Thunder","Dark"})Set(row,$"atk{target}DmgCorrectRate_{element}",1f);
        }
        foreach(var (key,value) in spec.fields)Set(row,key,value);
        return row;
    }
    static void Verify(PARAM effects) {
        var guardRows=effects.Rows.Where(r=>r.ID>=First&&r.ID<=First+2).ToArray();
        var guardIcons=guardRows.Where(r=>Convert.ToInt32(r["iconId"].Value)>=0).ToArray();
        if(guardIcons.Length!=1||guardIcons[0].ID!=First||Convert.ToSingle(guardIcons[0]["effectEndurance"].Value)!=-1f)
            throw new Exception("Guard HUD must use one resident icon, not the repeatedly recreated short effects");
        foreach(var spec in Rows) {
            var row=effects.Rows.Single(r=>r.ID==spec.id);var expected=Create(effects,spec);
            foreach(var cell in expected.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,row[cell.Def.InternalName].Value))throw new Exception($"Class effect mismatch: {spec.id} {cell.Def.InternalName}");
            if(Convert.ToInt32(row["spCategory"].Value)!=0)throw new Exception($"Class effect must not share a category: {spec.id}");
        }
        foreach(var resident in new[]{First+0,First+3,First+10,First+11,First+20,First+23,First+30,First+40})
            if(Convert.ToSingle(effects.Rows.Single(r=>r.ID==resident)["effectEndurance"].Value)!=-1f)throw new Exception($"Class effect must be resident: {resident}");
        if(Convert.ToInt32(effects.Rows.Single(r=>r.ID==First+21)["invocationConditionsStateChange1"].Value)!=Convert.ToInt32(effects.Rows.Single(r=>r.ID==First+20)["stateInfo"].Value))throw new Exception("Shadow trigger lost its critical-hit condition");
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        var existing=effects.Rows.Where(r=>r.ID>=First&&r.ID<First+100).ToList();
        // 本段只放职业效果：早期定义与现在不同的行原地替换，不再使用的行删除，新增的行追加
        var expected=Rows.Select(spec=>Create(effects,spec)).ToList();var ids=expected.Select(r=>r.ID).ToHashSet();
        var removed=effects.Rows.RemoveAll(r=>r.ID>=First&&r.ID<First+100&&!ids.Contains(r.ID));
        var (replaced,added)=(0,0);
        foreach(var row in expected) {
            int index=effects.Rows.FindIndex(r=>r.ID==row.ID);
            if(index<0){effects.Rows.Add(row);added++;}
            else if(!row.Cells.All(c=>System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,effects.Rows[index][c.Def.InternalName].Value))){effects.Rows[index]=row;replaced++;}
        }
        if(removed+replaced+added>0){effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();Console.WriteLine($"Class effects: {added} added, {replaced} replaced, {removed} removed");}
        Verify(effects);
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);Verify(actual);
        if(actual.Rows.Count!=original.Rows.Count-existing.Count+Rows.Length)throw new Exception("Unexpected effect count");
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows.Where(r=>r.ID<First||r.ID>=First+100))foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"class-build.json"),System.Text.Json.JsonSerializer.Serialize(new{effects=Rows.Select(r=>r.id),sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified {Rows.Length} class effects; all existing regulation data preserved");
    }
}
