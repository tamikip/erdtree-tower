using SoulsFormats;
// 余烬：战斗奖励里的机制增益。参数类效果放在 9670000 段，其余由运行时按事件施加。
// 无声处刑复制原版暗杀者匕首护符的三段触发（常驻 → 致命一击时触发 → 生效），保持相同的编号间隔；处刑之链用同样的触发留下短暂的标记，运行时据此加层。
// 叠层余烬不设上限：每种两行轮换（屠神者 +60、噬魂者 +70、处刑之链 +80 复制短剑护符的“只对致命一击生效”、灵魂收容 +90 挂在召唤物身上），
// 这里写一层的数值，运行时（native/src/embers.rs）按当前层数改写没挂着的那一行再换上。
static class EmberAssets {
    public const int First=9670000,Span=100;
    // 圣杯余响：喝圣杯瓶后造成的伤害 ×1.15、受到的伤害 ×0.85（与 src/embers.rs 的 ECHO_* 一致；持续时间由运行时控制）
    const float EchoAttack=1.15f,EchoCut=0.85f;
    // 法师余烬（与 src/embers.rs 的 BARRIER_* / GLINTSTONE_* / MOON_* 一致）：施法时受到的伤害 ×0.65、施法时韧性伤害 ×0（霸体）、
    // 法术与祷告的专注消耗 ×0.75（常驻）、专注不低于 70% 时造成的伤害 ×1.18；何时挂上由运行时控制
    const float BarrierCut=0.65f,MeditationToughness=0f,GlintstoneCost=0.75f,MoonAttack=1.18f;
    // 灵唤师的共鸣：召唤物在场时玩家造成的伤害 ×1.4（与 src/embers.rs 的 SPIRIT_RESONANCE_PERCENT 一致；何时挂上由运行时控制）
    const float ResonanceAttack=1.4f;
    // 战技的精粹（与 src/embers.rs 的 ARTS_* 一致）：战技造成的伤害 ×1.25（复制原版只对战技生效的增伤 312310，与战技伤害词条同一模板）、战技专注消耗 ×0.5；持有即常驻
    const float ArtsAttack=1.25f,ArtsCost=0.5f;
    const float ExecutionSeconds=5f,ExecutionStamina=0.5f,RevengeRate=1.3f,FrenzyStep=0.02f,CommandAttack=1.2f,CommandCut=0.8f;
    const int Buildup=12,FrenzyStacks=8,FlashVfx=1605000;
    // 叠层余烬（与 src/embers.rs 的常量一致）
    const float HarvestStep=0.01f;
    const float SlayerHp=0.04f,SlayerAttack=0.02f,SoulStep=0.02f,ChainStep=0.06f,KeeperAttack=0.03f,KeeperCut=0.02f;
    const int CriticalState=288,CriticalOnly=367;
    static readonly string[] Kept=["spCategory","categoryPriority","effectTargetOpposeTarget","effectTargetFriendlyTarget","effectTargetSelfTarget","effectTargetSelf","effectTargetFriend","effectTargetEnemy","effectTargetPlayer","effectTargetAI","effectTargetLive","effectTargetGhost"];
    static void Set(PARAM.Row r,string key,object value)=>r[key].Value=Convert.ChangeType(value,r[key].Value.GetType());
    static PARAM.Row Player(PARAM p,int id,string name,float attack,float endurance) {
        var r=new PARAM.Row(p.Rows.Single(r=>r.ID==321300)){ID=id,Name=name};
        foreach(var (key,value) in new (string,object)[]{("iconId",-1),("effectEndurance",endurance),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0),("magicSubCategoryChange1",0)})Set(r,key,value);
        foreach(var element in new[]{"physics","magic","fire","thunder","dark"})Set(r,element+"AttackRate",attack);
        foreach(var target in new[]{"Player","Enemy"})foreach(var element in new[]{"Physics","Magic","Fire","Thunder","Dark"})Set(r,$"atk{target}DmgCorrectRate_{element}",1f);
        return r;
    }
    static PARAM.Row Enemy(PARAM p,int id,string name,float endurance) {
        var neutral=p.Rows.Single(r=>r.ID==90400);
        var r=new PARAM.Row(p.Rows.Single(r=>r.ID==7030)){ID=id,Name=name};
        foreach(var cell in r.Cells.Where(c=>!Kept.Contains(c.Def.InternalName)))cell.Value=neutral[cell.Def.InternalName].Value;
        foreach(var (key,value) in new (string,object)[]{("effectEndurance",endurance),("spCategory",0),("categoryPriority",0),("saveCategory",-1)})Set(r,key,value);
        return r;
    }
    static List<PARAM.Row> Build(PARAM p) {
        var rows=new List<PARAM.Row>();
        // 无声处刑：常驻（stateInfo 288）→ 触发 → 致命一击后 5 秒精力消耗减半
        var resident=new PARAM.Row(p.Rows.Single(r=>r.ID==350500)){ID=First,Name="Tower ember execution"};
        Set(resident,"iconId",-1);Set(resident,"spCategory",0);Set(resident,"categoryPriority",0);rows.Add(resident);
        var trigger=new PARAM.Row(p.Rows.Single(r=>r.ID==350501)){ID=First+1,Name="Tower ember execution trigger"};
        Set(trigger,"replaceSpEffectId",First+2);Set(trigger,"spCategory",0);Set(trigger,"categoryPriority",0);rows.Add(trigger);
        // 生效的那一行由运行时在致命一击时直接挂上（原生处决事件），不再走处决状态 288：挂着 288 常驻时原版染红 / 染蓝凶刀的触发也会生效，处决会回血回蓝。
        // 用玩家增益的中性模板，只改持续时间、精力消耗与原来的光效
        var relief=Player(p,First+2,"Tower ember execution relief",1f,ExecutionSeconds);
        Set(relief,"consumeStaminaRate",ExecutionStamina);Set(relief,"vfxId",Convert.ToInt32(p.Rows.Single(r=>r.ID==350502)["vfxId"].Value));
        rows.Add(relief);
        // 异常积累复制原版鲜血油脂、冻结油脂的命中效果：积满后的发作内容（出血扣血、冻伤减防）由这一行提供，只改积累值
        var blood=new PARAM.Row(p.Rows.Single(r=>r.ID==3191)){ID=First+10,Name="Tower ember blood buildup"};Set(blood,"bloodAttackPower",Buildup);rows.Add(blood);
        var frost=new PARAM.Row(p.Rows.Single(r=>r.ID==3141)){ID=First+11,Name="Tower ember frost buildup"};Set(frost,"freezeAttackPower",Buildup);rows.Add(frost);
        rows.Add(Player(p,First+20,"Tower ember revenge",RevengeRate,-1f));
        var echo=Player(p,First+15,"Tower ember flask echo",EchoAttack,-1f);
        foreach(var cut in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(echo,cut+"DamageCutRate",EchoCut);
        rows.Add(echo);
        var barrier=Player(p,First+16,"Tower ember mage barrier",1f,-1f);
        foreach(var cut in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(barrier,cut+"DamageCutRate",BarrierCut);
        rows.Add(barrier);
        var meditation=Player(p,First+17,"Tower ember mage meditation",1f,-1f);Set(meditation,"toughnessDamageCutRate",MeditationToughness);rows.Add(meditation);
        var glintstone=Player(p,First+18,"Tower ember mage glintstone",1f,-1f);
        foreach(var kind in new[]{"magic","miracle"})Set(glintstone,kind+"ConsumptionRate",GlintstoneCost);
        rows.Add(glintstone);
        rows.Add(Player(p,First+19,"Tower ember mage full moon",MoonAttack,-1f));
        rows.Add(Player(p,First+21,"Tower ember spirit resonance",ResonanceAttack,-1f));
        var arts=new PARAM.Row(p.Rows.Single(r=>r.ID==312310)){ID=First+22,Name="Tower ember arts attack"};
        foreach(var (key,value) in new (string,object)[]{("iconId",-1),("effectEndurance",-1f),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0)})Set(arts,key,value);
        foreach(var element in new[]{"physics","magic","fire","thunder","dark"})Set(arts,element+"AttackRate",ArtsAttack);
        foreach(var target in new[]{"Player","Enemy"})foreach(var element in new[]{"Physics","Magic","Fire","Thunder","Dark"})Set(arts,$"atk{target}DmgCorrectRate_{element}",1f);
        rows.Add(arts);
        var artsCost=Player(p,First+23,"Tower ember arts cost",1f,-1f);Set(artsCost,"artsConsumptionRate",ArtsCost);rows.Add(artsCost);
        // 坩埚骑士的铁壁：格挡时的精力消耗 ×0（原版大盾护符 0.8）；最大负重 ×0.01（原版武器库护符 1.15），身上有装备就超重。整局常驻
        // 红狮子的不退：受到的削韧 ×1/3（强韧度 +200%；原版牛羊护符 0.75 即 +33%）。生命低于 40% 时由运行时挂上
        // 战斗法师：受到的伤害按智力 / 信仰取高每点 -0.4%，两行轮换，运行时按能力值改写（这里写 1 点的值）
        for(int slot=0;slot<2;slot++){var mind=Player(p,First+26+slot,$"Tower ember battlemage {slot}",1f,-1f);foreach(var cut in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(mind,cut+"DamageCutRate",0.996f);rows.Add(mind);}
        var lastStand=Player(p,First+25,"Tower ember last stand",1f,-1f);Set(lastStand,"toughnessDamageCutRate",1f/3f);rows.Add(lastStand);
        var bulwark=Player(p,First+24,"Tower ember bulwark",1f,-1f);Set(bulwark,"guardStaminaMult",0f);Set(bulwark,"equipWeightChangeRate",0.01f);rows.Add(bulwark);
        for(int stack=1;stack<=FrenzyStacks;stack++)rows.Add(Player(p,First+30+stack,$"Tower ember frenzy {stack}",1f+FrenzyStep*stack,-1f));
        var flash=Enemy(p,First+40,"Tower ember flash",1f);Set(flash,"vfxId",FlashVfx);rows.Add(flash);
        // 灵唤铃的号令：挂在召唤物身上，伤害 +20%、受到的伤害 -20%（沿用敌人侧的中性模板）
        var command=Enemy(p,First+50,"Tower ember spirit command",-1f);
        foreach(var element in new[]{"physics","magic","fire","thunder","dark"})Set(command,element+"AttackPowerRate",CommandAttack);
        foreach(var cut in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(command,cut+"DamageCutRate",CommandCut);
        rows.Add(command);
        // 处刑之链：常驻 → 致命一击时触发 → 1 秒的标记（没有任何效果，只供运行时识别）
        var chain=new PARAM.Row(p.Rows.Single(r=>r.ID==350500)){ID=First+5,Name="Tower ember chain"};
        Set(chain,"iconId",-1);Set(chain,"spCategory",0);Set(chain,"categoryPriority",0);rows.Add(chain);
        var chainTrigger=new PARAM.Row(p.Rows.Single(r=>r.ID==350501)){ID=First+6,Name="Tower ember chain trigger"};
        Set(chainTrigger,"replaceSpEffectId",First+7);Set(chainTrigger,"spCategory",0);Set(chainTrigger,"categoryPriority",0);rows.Add(chainTrigger);
        var mark=new PARAM.Row(p.Rows.Single(r=>r.ID==350502)){ID=First+7,Name="Tower ember chain mark"};
        foreach(var (key,value) in new (string,object)[]{("changeHpRate",0f),("changeHpPoint",0),("motionInterval",0f),("effectEndurance",1f),("vfxId",-1),("spCategory",0),("categoryPriority",0)})Set(mark,key,value);
        rows.Add(mark);
        for(int slot=0;slot<2;slot++) {
            var slayer=Player(p,First+60+slot,$"Tower ember godslayer {slot}",1f+SlayerAttack,-1f);Set(slayer,"maxHpRate",1f+SlayerHp);rows.Add(slayer);
            // 处刑者的血宴：只加最大生命（一点的倍率，运行时按点数改写成 1.01^点数）
            var harvest=Player(p,First+62+slot,$"Tower ember harvest {slot}",1f,-1f);Set(harvest,"maxHpRate",1f+HarvestStep);rows.Add(harvest);
            rows.Add(Player(p,First+70+slot,$"Tower ember soul eater {slot}",1f+SoulStep,-1f));
            var critical=Player(p,First+80+slot,$"Tower ember chain {slot}",1f+ChainStep,-1f);
            Set(critical,"stateInfo",CriticalOnly);Set(critical,"throwAttackParamChange",1);rows.Add(critical);
            var keeper=Enemy(p,First+90+slot,$"Tower ember spirit keeper {slot}",-1f);
            foreach(var element in new[]{"physics","magic","fire","thunder","dark"})Set(keeper,element+"AttackPowerRate",1f+KeeperAttack);
            foreach(var cut in new[]{"slash","blow","thrust","neutral","magic","fire","thunder","dark"})Set(keeper,cut+"DamageCutRate",1f-KeeperCut);
            rows.Add(keeper);
        }
        return rows;
    }
    static void Verify(PARAM p) {
        foreach(var expected in Build(p)) {
            var actual=p.Rows.Single(r=>r.ID==expected.ID);
            foreach(var cell in expected.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,actual[cell.Def.InternalName].Value))throw new Exception($"Ember effect mismatch: {expected.ID} {cell.Def.InternalName}");
        }
        foreach(var (resident,trigger,result) in new[]{(First,First+1,First+2),(First+5,First+6,First+7)}) {
            var t=p.Rows.Single(r=>r.ID==trigger);
            if(Convert.ToInt32(p.Rows.Single(r=>r.ID==resident)["stateInfo"].Value)!=CriticalState||Convert.ToInt32(t["invocationConditionsStateChange1"].Value)!=CriticalState||Convert.ToInt32(t["replaceSpEffectId"].Value)!=result)
                throw new Exception($"Critical-hit trigger {trigger} lost its condition");
        }
        // 无声处刑生效行：不回血回蓝、不依赖处决状态（直接由运行时挂上）
        var relief=p.Rows.Single(r=>r.ID==First+2);
        if(new[]{"changeHpRate","changeHpPoint","changeMpRate","changeMpPoint","invocationConditionsStateChange1","replaceSpEffectId","cycleOccurrenceSpEffectId"}.Any(f=>Convert.ToDouble(relief[f].Value)!=(f is "replaceSpEffectId" or "cycleOccurrenceSpEffectId"?-1:0)))
            throw new Exception("Execution relief must only cut stamina cost");
        if(new[]{First+80,First+81}.Any(id=>Convert.ToInt32(p.Rows.Single(r=>r.ID==id)["throwAttackParamChange"].Value)!=1))throw new Exception("Chain stacks must only boost critical hits");
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="SpEffectParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var effects=PARAM.Read(file.Bytes);effects.ApplyParamdef(def);
        var count=Build(effects).Count;
        var existing=effects.Rows.Count(r=>r.ID>=First&&r.ID<First+Span);
        var expectedIds=Build(effects).Select(r=>r.ID).ToHashSet();
        if(effects.Rows.Any(r=>r.ID>=First&&r.ID<First+Span&&!expectedIds.Contains(r.ID)))throw new Exception($"Ember effect range {First}-{First+Span-1} already in use");
        // 早期版本的行与定义不同时原地换成新定义（出血、冻伤补上发作内容），后来新增的余烬效果追加进来
        var (replaced,added)=(0,0);
        foreach(var expected in Build(effects)) {
            int index=effects.Rows.FindIndex(r=>r.ID==expected.ID);
            if(index<0){effects.Rows.Add(expected);added++;}
            else if(!expected.Cells.All(c=>System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,effects.Rows[index][c.Def.InternalName].Value))){effects.Rows[index]=expected;replaced++;}
        }
        if(replaced+added>0){effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=effects.Write();Console.WriteLine($"Ember effects: {added} added, {replaced} replaced");}
        Verify(effects);
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);Verify(actual);
        if(actual.Rows.Count!=original.Rows.Count+(count-existing))throw new Exception("Unexpected effect count");
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows.Where(r=>r.ID<First||r.ID>=First+Span))foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))throw new Exception($"Existing effect changed: {row.ID}");
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"ember-build.json"),System.Text.Json.JsonSerializer.Serialize(new{effects=actual.Rows.Where(r=>r.ID>=First&&r.ID<First+Span).Select(r=>r.ID),sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path)},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified {count} ember effects; all existing regulation data preserved");
    }
}
