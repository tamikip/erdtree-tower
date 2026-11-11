using SoulsFormats;
// 爬塔敌人强度：去掉原版按产地附带的区域强度，按「幕 × 弱怪/精英/Boss」换成 tower-scaling.txt 中的统一档位；
// 档位用爬塔自己的区域强度效果 TierFirst+档位×10：复制原版 7000+档位×10，生命等不变，只把攻击倍率乘 TierAttackScale
// （原版行不改，原版地图上的敌人照旧）。每层递增由运行时施加 9640001–9640011。
static class TowerScalingAssets {
    public const int GrowthFirst=9640000,GrowthSteps=11;
    public const float GrowthHp=0.03f,GrowthAttack=0.015f;
    // 旧 Boss 专属加成，迁移时移除引用及参数行。
    public const int BossBonus=9640050;
    // 本体区域强度 7000–7280；DLC 的区域强度 20007000–20007150（敌人）与 20007200–20007350（人形 NPC），之后追加的 DLC Boss 带着它们
    // 爬塔的档位效果（TierFirst 起）也算区域强度，重跑时会被替换
    public static bool AreaScaling(int id)=>id>=7000&&id<=7280||id>=20007000&&id<=20007350||id>=TierFirst&&id<TierFirst+300;
    public const int TierFirst=9641000;
    /// 档位攻击倍率：原版区域强度的攻击倍率再乘这个系数（全局降伤，生命不变）
    public const float TierAttackScale=0.8f;
    public static int TierEffect(int tier)=>TierFirst+tier*10;
    static PARAM.Row TowerTier(PARAM effects,int tier) {
        var row=new PARAM.Row(effects.Rows.Single(r=>r.ID==7000+tier*10)){ID=TierEffect(tier),Name=$"Tower area scaling tier {tier}"};
        foreach(var key in Attacks)row[key].Value=Convert.ToSingle(row[key].Value)*TierAttackScale;
        return row;
    }
    static readonly string[] Attacks=["physicsAttackPowerRate","magicAttackPowerRate","fireAttackPowerRate","thunderAttackPowerRate","darkAttackPowerRate"];
    public record Target(int Id,int Act,string Role,string Name,bool Room=false);
    public static Dictionary<int,int[]> Tiers(string input)=>File.ReadAllLines(Path.Combine(input,"tower-scaling.txt")).Where(l=>!l.StartsWith('#')&&l.Length>0)
        .Select(l=>l.Split('|').Select(int.Parse).ToArray()).ToDictionary(p=>p[0],p=>p[1..]);
    public static List<Target> Targets(string input) {
        var targets=new List<Target>();
        foreach(var p in File.ReadAllLines(Path.Combine(input,"enemy-pools.txt")).Select(l=>l.Split('|')))
            targets.Add(new(900000000+int.Parse(p[2])*10000,int.Parse(p[0]),p[1] switch {"weak"=>"weak","elite"=>"elite",_=>"boss"},p[4]));
        foreach(var p in BossAssets.Parts(input)) {targets.Add(new(BossAssets.Npc(p,false),p.Act,"boss",p.Name,true));targets.Add(new(BossAssets.Npc(p,true),p.Index==54||p.Name=="坠星成兽"||p.Name=="英雄石像鬼双人组"?3:p.Act,"elite",p.Name));}
        return targets;
    }
    static int Tier(Dictionary<int,int[]> tiers,Target t)=>tiers[t.Act][t.Role switch {"weak"=>0,"elite"=>1,_=>2}];
    static string[] Slots(PARAM.Row row)=>row.Cells.Select(c=>c.Def.InternalName).Where(n=>n.StartsWith("spEffectID")&&int.TryParse(n[10..],out _)).ToArray();
    static int I(PARAM.Row row,string field)=>Convert.ToInt32(row[field].Value);
    public static void Apply(PARAM.Row row,int tier) {
        var slots=Slots(row);
        foreach(var slot in slots.Where(s=>AreaScaling(I(row,s))))row[slot].Value=Convert.ChangeType(-1,row[slot].Value.GetType());
        var free=slots.FirstOrDefault(s=>I(row,s)==-1)??throw new Exception($"No free SpEffect slot on NpcParam {row.ID}");
        row[free].Value=Convert.ChangeType(TierEffect(tier),row[free].Value.GetType());
    }
    static PARAM.Row Growth(PARAM effects,int step) {
        var neutral=effects.Rows.Single(r=>r.ID==90400);
        var row=new PARAM.Row(effects.Rows.Single(r=>r.ID==7030)){ID=GrowthFirst+step,Name=$"Tower layer growth {step}"};
        string[] kept=["spCategory","categoryPriority","effectTargetOpposeTarget","effectTargetFriendlyTarget","effectTargetSelfTarget","effectTargetSelf","effectTargetFriend","effectTargetEnemy","effectTargetPlayer","effectTargetAI","effectTargetLive","effectTargetGhost","bCurrHPIndependeMaxHP"];
        foreach(var cell in row.Cells.Where(c=>!kept.Contains(c.Def.InternalName)))cell.Value=neutral[cell.Def.InternalName].Value;
        foreach(var (key,value) in new (string,object)[]{("effectEndurance",-1f),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("maxHpRate",1f+GrowthHp*step)})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
        foreach(var key in Attacks)row[key].Value=1f+GrowthAttack*step;
        return row;
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        PARAM Read(string name,string def,out BinderFile file) {file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);var p=PARAM.Read(file.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{def}.xml"));return p;}
        var npcs=Read("NpcParam","NpcParam",out var npcFile);var effects=Read("SpEffectParam","SpEffect",out var effectFile);
        var before=npcs.Rows.ToDictionary(r=>r.ID,r=>new PARAM.Row(r));var beforeEffects=effects.Rows.Count;
        var tiers=Tiers(input);var targets=Targets(input);
        foreach(var g in targets.GroupBy(t=>t.Id))if(g.Select(t=>Tier(tiers,t)).Distinct().Count()>1)throw new Exception($"NpcParam {g.Key} is shared by enemies with different tiers");
        targets=targets.GroupBy(t=>t.Id).Select(g=>g.First()).ToList();var report=new List<string>{"npc\tact\trole\tname\tbase_hp\told_tier\tnew_tier\told_hp\tnew_hp_layer1\tnew_hp_last_layer\told_attack\tnew_attack_layer1\tnew_attack_last_layer"};
        // 报告里的“原来”按替换前的效果算
        var previous=effects.Rows.Where(r=>AreaScaling(r.ID)).ToDictionary(r=>r.ID,r=>new PARAM.Row(r));
        // 爬塔档位效果：用到的档位各一行，已有的替换（攻击系数改了就重算）
        effects.Rows.RemoveAll(r=>r.ID>=TierFirst&&r.ID<TierFirst+300);
        foreach(var tier in tiers.Values.SelectMany(v=>v).Distinct().OrderBy(t=>t))effects.Rows.Add(TowerTier(effects,tier));
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        var area=effects.Rows.Where(r=>AreaScaling(r.ID)).ToDictionary(r=>r.ID);
        foreach(var t in targets) {
            var row=npcs.Rows.Single(r=>r.ID==t.Id);int tier=Tier(tiers,t);
            if(tier<1||tier>21)throw new Exception($"Invalid tier {tier} for act {t.Act} {t.Role}");
            var old=Slots(row).Select(s=>I(row,s)).Where(AreaScaling).ToList();
            Apply(row,tier);
            foreach(var slot in Slots(row).Where(s=>I(row,s)==BossBonus))row[slot].Value=Convert.ChangeType(-1,row[slot].Value.GetType());
            int hp=I(row,"hp");int oldId=old.FirstOrDefault(-1);int newId=TierEffect(tier);
            float Hp(int id)=>(area.TryGetValue(id,out var e)||previous.TryGetValue(id,out e))?Convert.ToSingle(e["maxHpRate"].Value):1f;
            float Atk(int id)=>id==oldId&&previous.TryGetValue(id,out var p)?Convert.ToSingle(p["physicsAttackPowerRate"].Value):area.TryGetValue(id,out var e)?Convert.ToSingle(e["physicsAttackPowerRate"].Value):1f;
            report.Add(string.Join('\t',t.Id,t.Act,t.Role,t.Name,hp,oldId<0?0:oldId>=TierFirst?(oldId-TierFirst)/10:(oldId-7000)/10,tier,(int)(hp*Hp(oldId)),(int)(hp*Hp(newId)),(int)(hp*Hp(newId)*(1+GrowthHp*GrowthSteps)),
                Atk(oldId).ToString("0.00"),Atk(newId).ToString("0.00"),(Atk(newId)*(1+GrowthAttack*GrowthSteps)).ToString("0.00")));
        }
        if(effects.Rows.Any(r=>r.ID>GrowthFirst&&r.ID<=GrowthFirst+GrowthSteps)) {
            for(int step=1;step<=GrowthSteps;step++){var expected=Growth(effects,step);var actual=effects.Rows.Single(r=>r.ID==GrowthFirst+step);foreach(var c in expected.Cells)if(!Equals(c.Value,actual[c.Def.InternalName].Value))throw new Exception($"Growth effect mismatch {actual.ID}");}
        } else {
            if(effects.Rows.Any(r=>r.ID>=GrowthFirst&&r.ID<GrowthFirst+100))throw new Exception("Growth effect range 9640000-9640099 already in use");
            for(int step=1;step<=GrowthSteps;step++)effects.Rows.Add(Growth(effects,step));
            effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        }
        // 不再叠加 Boss 房专属生命和攻击倍率。
        effects.Rows.RemoveAll(r=>r.ID==BossBonus);
        if(effects.Rows.Any(r=>r.ID>GrowthFirst+GrowthSteps&&r.ID<GrowthFirst+100&&r.ID!=BossBonus))throw new Exception("Boss bonus range already in use");
        effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        npcFile.Bytes=npcs.Write();effectFile.Bytes=effects.Write();
        var others=reg.Files.Where(f=>f!=npcFile&&f!=effectFile).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>others.ContainsKey(f.Name)))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var after=PARAM.Read(check.Files.Single(f=>f.Name==npcFile.Name).Bytes);after.ApplyParamdef(npcs.AppliedParamdef);
        var ids=targets.Select(t=>t.Id).ToHashSet();
        foreach(var row in after.Rows) {
            var old=before[row.ID];
            foreach(var cell in row.Cells) {
                bool ours=ids.Contains(row.ID)&&cell.Def.InternalName.StartsWith("spEffectID");
                if(!ours&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,old[cell.Def.InternalName].Value))throw new Exception($"Unexpected NpcParam change: {row.ID} {cell.Def.InternalName}");
            }
            if(ids.Contains(row.ID)) {
                var slots=Slots(row);var t=targets.First(x=>x.Id==row.ID);
                if(slots.Select(s=>I(row,s)).Count(AreaScaling)!=1||!slots.Any(s=>I(row,s)==TierEffect(Tier(tiers,t))))throw new Exception($"Scaling not applied: {row.ID}");
                foreach(var s in slots)if(!AreaScaling(I(row,s))&&!AreaScaling(I(old,s))&&I(row,s)!=I(old,s)&&!(I(old,s)==BossBonus&&I(row,s)==-1))throw new Exception($"Non-scaling effect changed: {row.ID} {s}");
                if(slots.Any(s=>I(row,s)==BossBonus))throw new Exception($"Retired Boss bonus still attached: {row.ID}");
            }
        }
        var effectsCheck=PARAM.Read(check.Files.Single(f=>f.Name==effectFile.Name).Bytes);effectsCheck.ApplyParamdef(effects.AppliedParamdef);
        if(effectsCheck.Rows.Count!=effects.Rows.Count||Enumerable.Range(1,GrowthSteps).Any(s=>!effectsCheck.Rows.Any(r=>r.ID==GrowthFirst+s)))throw new Exception("Growth effects missing");
        if(effectsCheck.Rows.Any(r=>r.ID==BossBonus)||after.Rows.Any(r=>Slots(r).Any(s=>I(r,s)==BossBonus)))throw new Exception("Retired Boss bonus remains in regulation");
        File.WriteAllLines(Path.Combine(output,"tower-scaling-report.tsv"),report);
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"tower-scaling-build.json"),System.Text.Json.JsonSerializer.Serialize(new{targets=targets.Count,growthEffects=GrowthSteps,growthHpPerStep=GrowthHp,growthAttackPerStep=GrowthAttack,sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Normalized area scaling on {targets.Count} tower NPC rows ({effects.Rows.Count-beforeEffects} growth effects added); other params preserved; report: tower-scaling-report.tsv");
    }
}
