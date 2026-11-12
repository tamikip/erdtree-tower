using SoulsFormats;
static class BossAffixAssets {
    public const int First=9620000;
    const int Resident=7030,Neutral=90400,PlayerStatus=4051,VanillaRot=21001;
    // 黄金护盾的全身金光：原版黄金壁垒（1633000）与黄金雷电附加（1790300）的复制，只把播放优先级提到最高。
    // 全身光效（playCategory 3）同一时间只显示优先级最高的一个，原版这两个是 70，与傲慢的白金光、嫉妒的黑焰一样高，
    // Boss 同时带这些词条时先挂上的一直占着，护盾展开时金光出不来。原版行不改（玩家自己的黄金誓言等照旧）
    static readonly (int id,int source,byte priority)[] ShieldVfx=[(9620090,1633000,100),(9620091,1790300,99)];
    static readonly string[] Cuts=["slashDamageCutRate","blowDamageCutRate","thrustDamageCutRate","neutralDamageCutRate","magicDamageCutRate","fireDamageCutRate","thunderDamageCutRate","darkDamageCutRate"];
    static readonly string[] Attacks=["physicsAttackPowerRate","magicAttackPowerRate","fireAttackPowerRate","thunderAttackPowerRate","darkAttackPowerRate"];
    static readonly string[] Kept=["spCategory","categoryPriority","effectTargetOpposeTarget","effectTargetFriendlyTarget","effectTargetSelfTarget","effectTargetSelf","effectTargetFriend","effectTargetEnemy","effectTargetPlayer","effectTargetAI","effectTargetLive","effectTargetGhost"];
    static (string,object)[] All(string[] fields,float value)=>fields.Select(f=>(f,(object)value)).ToArray();
    // kind：0 挂在 Boss 身上（清空为中性的 7030），1 给玩家的一次性积累（4051），2 给玩家的短时效果（复制苦难律法玩家侧用过的 321300），
    // 3 原版“NPC：猩红腐败”（21001）原样复制：同一行里既有积累值、也有积满后 180 秒的持续扣血
    // 21001 原本只能作用于“别人”（effectTargetSelf / SelfTarget 为 0），由原生 DLL 直接挂到玩家身上时会被忽略，所以打开这两项
    static (int id,string name,int kind,(string,object)[] fields)[] Rows=>[
        (First,"Tower boss affix stalwart",0,[("toughnessDamageCutRate",0.5f),("vfxId",8570)]),
        (First+10,"Tower boss affix bulwark",0,[..All(Cuts,0.75f),("vfxId",8510)]),
        (First+20,"Tower boss affix frenzied",0,[..All(Attacks,1.3f),..All(Cuts,1.1f),("vfxId",1605000)]),
        (First+30,"Tower boss affix regenerating",0,[("motionInterval",1f),("changeHpRate",-0.3f),("vfxId",1643000)]),
        // 苦难律法「词条强化」：编号 +1 的强化版，光效相同（削韧 ×0.35、受到伤害 -35%、攻击 +35%、每秒回复 0.45%）
        (First+1,"Tower boss affix stalwart empowered",0,[("toughnessDamageCutRate",0.35f),("vfxId",8570)]),
        (First+11,"Tower boss affix bulwark empowered",0,[..All(Cuts,0.65f),("vfxId",8510)]),
        (First+21,"Tower boss affix frenzied empowered",0,[..All(Attacks,1.35f),..All(Cuts,1.1f),("vfxId",1605000)]),
        (First+31,"Tower boss affix regenerating empowered",0,[("motionInterval",1f),("changeHpRate",-0.45f),("vfxId",1643000)]),
        (First+40,"Tower boss affix rot aura",0,[("vfxId",5)]),
        (First+42,"Tower boss affix rot buildup (vanilla scarlet rot, self)",3,[("effectTargetSelf",1),("effectTargetSelfTarget",1)]),
        // 迅猛、荆棘整场挂着；吸血在每次回血时闪一下。每个词条叠 2～3 个原版的全身光效，颜色互相区分：
        // 迅猛（风之结晶露滴 + 刺客的手法 + 野兽的活力）、荆棘（荆棘斗篷 + 鲜血君王的欢愉 + 荆棘龟裂露滴）、吸血（血税 + 吸血龟裂露滴）
        (First+50,"Tower boss affix swift",0,[("vfxId",5846),("vfxId1",1651000),("vfxId2",1685000)]),
        (First+60,"Tower boss affix thorned",0,[("vfxId",21490001),("vfxId1",321600),("vfxId2",5865)]),
        (First+65,"Tower boss affix leeching flash",0,[("effectEndurance",1.5f),("vfxId",8680),("vfxId1",5806)]),
        // 嫉妒用黑焰（黑焰庇佑 + 黑焰缠身），傲慢用白金圣光（王之神圣加护 + 神圣加护），与狂暴的红光、黄金律法的金光区分开
        (First+70,"Tower boss affix envious rage",0,[("effectEndurance",15f),..All(Attacks,1.3f),("vfxId",1627000),("vfxId1",9010)]),
        (First+80,"Tower boss affix proud",0,[..All(Attacks,1.2f),("vfxId",1649000),("vfxId1",1648000)]),
        // 词条强化：嫉妒攻击 +45%、傲慢攻击 +30%
        (First+71,"Tower boss affix envious rage empowered",0,[("effectEndurance",15f),..All(Attacks,1.45f),("vfxId",1627000),("vfxId1",9010)]),
        (First+81,"Tower boss affix proud empowered",0,[..All(Attacks,1.3f),("vfxId",1649000),("vfxId1",1648000)]),
        // 黄金律法的护盾：同时挂三个原版金色护体光效（黄金壁垒、黄金雷电附加、黄金誓言），比单个黄金誓言醒目
        (First+90,"Tower boss affix golden shield",0,[("effectEndurance",5f),..All(Cuts,0.3f),("vfxId",9620090),("vfxId1",9620091),("vfxId2",1660000)]),
        (First+95,"Tower boss affix reborn",0,[("effectEndurance",3f),("vfxId",1643000)]),
    ];
    static PARAM.Row Create(PARAM effects,(int id,string name,int kind,(string,object)[] fields) spec) {
        PARAM.Row row;
        if(spec.kind==3) {
            row=new PARAM.Row(effects.Rows.Single(r=>r.ID==VanillaRot)){ID=spec.id,Name=spec.name};
        } else if(spec.kind==2) {
            row=new PARAM.Row(effects.Rows.Single(r=>r.ID==321300)){ID=spec.id,Name=spec.name};
            foreach(var (key,value) in new (string,object)[]{("iconId",-1),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0),("magicSubCategoryChange1",0)})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            foreach(var element in new[]{"physics","magic","fire","thunder","dark"})row[element+"AttackRate"].Value=1f;
            foreach(var target in new[]{"Player","Enemy"})foreach(var element in new[]{"Physics","Magic","Fire","Thunder","Dark"})row[$"atk{target}DmgCorrectRate_{element}"].Value=1f;
        } else if(spec.kind==1) {
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
            if(spec.kind!=1&&spec.kind!=3&&Convert.ToInt32(row["spCategory"].Value)!=0)throw new Exception($"Boss affix effect must not share a stacking category: {spec.id}");
        }
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        // 已打过补丁时原地更新、追加新词条的效果；97、98、99 是 text-update 写入的血条名称标记，保留不动
        // 本段只放 Boss 词条效果：不再使用的行删除（97–99 三个标记除外）
        var ids=Rows.Select(r=>r.id).Concat([First+97,First+98,First+99]).ToHashSet();
        var existing=effects.Rows.Count(r=>r.ID>=First&&r.ID<First+97);
        var removed=effects.Rows.RemoveAll(r=>r.ID>=First&&r.ID<First+97&&!ids.Contains(r.ID));
        var (added,replaced)=(0,0);
        foreach(var spec in Rows) {
            var row=Create(effects,spec);int index=effects.Rows.FindIndex(r=>r.ID==spec.id);
            if(index<0){effects.Rows.Add(row);added++;}
            else if(!row.Cells.All(c=>System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,effects.Rows[index][c.Def.InternalName].Value))){effects.Rows[index]=row;replaced++;}
        }
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();
        Console.WriteLine($"Boss affix effects: {added} added, {replaced} replaced, {removed} removed");
        var vfxFile=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectVfxParam");
        var vfxDef=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffectVfx.xml");
        var vfxOriginal=PARAM.Read(vfxFile.Bytes);vfxOriginal.ApplyParamdef(vfxDef);
        var vfx=PARAM.Read(vfxFile.Bytes);vfx.ApplyParamdef(vfxDef);
        vfx.Rows.RemoveAll(r=>ShieldVfx.Any(v=>v.id==r.ID));
        foreach(var (id,source,priority) in ShieldVfx) {var row=new PARAM.Row(vfx.Rows.Single(r=>r.ID==source)){ID=id,Name="Tower boss golden shield"};row["playPriority"].Value=priority;vfx.Rows.Add(row);}
        vfx.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));vfxFile.Bytes=vfx.Write();
        var others=reg.Files.Where(f=>f!=file&&f!=vfxFile).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name&&f.Name!=vfxFile.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var vfxActual=PARAM.Read(check.Files.Single(f=>f.Name==vfxFile.Name).Bytes);vfxActual.ApplyParamdef(vfxDef);var vfxRows=vfxActual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in vfxOriginal.Rows.Where(r=>!ShieldVfx.Any(v=>v.id==r.ID)))foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,vfxRows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing vfx changed: {row.ID}");
        foreach(var (id,source,priority) in ShieldVfx) {
            var row=vfxRows[id];
            foreach(var cell in row.Cells)if(cell.Def.InternalName!="playPriority"&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,vfxRows[source][cell.Def.InternalName].Value))throw new Exception($"Shield vfx differs from its source: {id} {cell.Def.InternalName}");
            if(Convert.ToByte(row["playPriority"].Value)!=priority)throw new Exception($"Shield vfx priority not raised: {id}");
        }
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);Verify(actual);
        if(actual.Rows.Count!=original.Rows.Count-existing+Rows.Length)throw new Exception("Unexpected effect count");
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows.Where(r=>r.ID<First||r.ID>=First+97))foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"boss-affix-build.json"),System.Text.Json.JsonSerializer.Serialize(new{effects=Rows.Select(r=>r.id),sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified {Rows.Length} boss affix effects; all existing regulation data preserved");
    }
}
