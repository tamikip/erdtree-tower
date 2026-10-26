using SoulsFormats;
using System.Globalization;

static class AdvancedAffixAssets {
    const int WeaponFirst=160000000,ArmorFirst=20000000,CorrectionFirst=9860000,EffectFirst=9850000;
    static readonly string[] Tiers=["普通","稀有","史诗","传说"];
    static readonly string[] Types=["物理","魔法","锈蚀","艾尔登"];
    static readonly string[] Stats=["Strength","Dexterity","Magic","Faith","Luck"];
    static readonly string[] Scaling=["correctStrength","correctAgility","correctMagic","correctFaith","correctLuck"];
    static readonly string[] Labels=["力气","灵巧","智力","信仰","感应"];
    static readonly string[] Damage=["Physics","Magic","Fire","Thunder","Dark"];
    static readonly string[] DamageLabels=["物理","魔力","火焰","雷电","神圣"];
    static ulong Mix(ulong n){unchecked{n=(n^(n>>30))*0xbf58476d1ce4e5b9UL;n=(n^(n>>27))*0x94d049bb133111ebUL;return n^(n>>31);}}
    static void Set(PARAM.Row r,string k,object v)=>r[k].Value=Convert.ChangeType(v,r[k].Value.GetType(),CultureInfo.InvariantCulture);
    static int I(PARAM.Row r,string k)=>Convert.ToInt32(r[k].Value);
    static double F(PARAM.Row r,string k)=>Convert.ToDouble(r[k].Value);
    static int Mask(PARAM.Row source,ulong seed,int profile) {
        int anchor=profile==0?Enumerable.Range(0,5).MaxBy(i=>F(source,Scaling[i])):(int)(Mix(seed^19)%5);
        int mask=1<<anchor;
        if(Mix(seed^23)%100<70)mask|=1<<(int)(Mix(seed^29)%5);
        if(Mix(seed^31)%100<20)mask|=1<<(int)(Mix(seed^37)%5);
        if(Mix(seed^41)%100<5)mask=31;
        if(I(source,"enableMagic")!=0)mask|=4;
        if(I(source,"enableMiracle")!=0)mask|=8;
        return mask;
    }
    static double ArmorMultiplier(int type,bool physical)=>type switch {
        0=>physical?1.6:0,1=>physical?0:1.8,2=>0.55,_=>1.2
    };
    public static void Test() {
        foreach(bool physical in new[]{false,true}) {
            if(ArmorMultiplier(2,physical)>=ArmorMultiplier(3,physical))throw new Exception("Rusted/Elden ordering mismatch");
        }
        if(ArmorMultiplier(0,false)!=0||ArmorMultiplier(1,true)!=0)throw new Exception("Armor specialization mismatch");
        if(ArmorMultiplier(0,true)<=ArmorMultiplier(3,true)||ArmorMultiplier(1,false)<=ArmorMultiplier(3,false))throw new Exception("Armor specialization must exceed balanced armor");
        Console.WriteLine("Verified independent physical/magical/rusted/elden armor rules");
    }
    public static void Build(Func<string,PARAM> read,Action<string,PARAM> save,BND4 messages,string input,string output) {
        Test();
        var correction=read("AttackElementCorrectParam");
        if(correction.Rows.Any(r=>r.ID>CorrectionFirst&&r.ID<=CorrectionFirst+31))throw new Exception("Advanced scaling correction collision");
        for(int mask=1;mask<32;mask++) {
            var row=new PARAM.Row(correction.Rows.Single(r=>r.ID==ScalingCorrectionFix.Template)){ID=CorrectionFirst+mask,Name=$"Tower scaling mask {mask}"};
            for(int stat=0;stat<5;stat++)foreach(var element in Damage)Set(row,$"is{Stats[stat]}Correct_by{element}",(mask>>stat)&1);
            correction.Rows.Add(row);
        }
        save("AttackElementCorrectParam",correction);
        var effects=read("SpEffectParam");save("SpEffectParam",effects);
        var text=messages.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes));
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var original=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).ToArray();
        var catalog=new List<string>();var detailLines=new List<string>();var rarities=new List<string>();var audit=new List<string>{"ID\tBaseID\tKind\tTier\tProfile\tMask\tDamageMode\tStars\tEffects"};
        foreach(var kind in new[]{"weapon","armor"}) {
            bool weapon=kind=="weapon";uint tag=weapon?0u:0x10000000u;int profiles=weapon?2:4;
            string table=weapon?"EquipParamWeapon":"EquipParamProtector";
            var p=read(table);var old=p.Rows.ToArray();var occupied=old.Select(r=>r.ID).ToHashSet();int ordinal=0;
            foreach(var entry in original.Where(e=>e[0]==kind).OrderBy(e=>int.Parse(e[1]))) {
                int baseId=int.Parse(entry[1]);var source=old.Single(r=>r.ID==baseId);var detail=details[tag|(uint)baseId];
                var family=weapon?old.Where(r=>r.ID>=baseId&&r.ID<baseId+10000).ToArray():[source];
                for(int profile=0;profile<profiles;profile++)for(int tier=0;tier<4;tier++) {
                    int id=(weapon?WeaponFirst:ArmorFirst)+(ordinal*profiles*4+profile*4+tier)*(weapon?10000:1000);
                    ulong seed=Mix((ulong)baseId^((ulong)tier<<32)^((ulong)profile<<40)^0xaff22026UL);
                    double roll=(Mix(seed^43)%10001)/10000.0;int mask=weapon?Mask(source,seed,profile):0;
                    int mode=(int)(Mix(seed^47)%5),stars=roll>=0.995?2:roll>=0.97?1:0;
                    string build=mask==31?"全属性":string.Join('/',Labels.Where((_,i)=>(mask&(1<<i))!=0));
                    string name=weapon?$"{Tiers[tier]}·{build}·{entry[2]}{(stars==0?"":stars==1?" ★":" ★★")}":$"{Tiers[tier]}·{Types[profile]}·{entry[2]}";
                    var slots=weapon?new[]{"residentSpEffectId","residentSpEffectId1","residentSpEffectId2"}:new[]{"residentSpEffectId","residentSpEffectId2","residentSpEffectId3"};
                    int count=Math.Min(tier<2?1:tier,family.Min(r=>slots.Count(k=>I(r,k)<=0)));
                    var choices=Enumerable.Range(0,12).OrderBy(i=>Mix(seed^(ulong)(100+i))).Take(count).ToArray();
                    string effectText=string.Join("\\n",choices.Select(i=>AffixAssets.DescribeEffect(i,tier)));
                    if(count==0)effectText="保留全部固有效果";
                    string info=(weapon?$"补正组合：{build}":$"防具类型：{Types[profile]}\\n"+(profile switch {0=>"偏重物理防护，不提供元素减伤",1=>"偏重元素防护，不提供物理减伤",2=>"物理与元素防护较低，重量较轻",_=>"兼顾物理与元素防护"}))+"\\n"+effectText;
                    PARAM.Row basis=null;
                    foreach(var originalRow in family) {
                        var row=new PARAM.Row(originalRow){ID=id+originalRow.ID-baseId,Name=$"Tower advanced affix {baseId}:{profile}:{tier}"};
                        if(!occupied.Add(row.ID))throw new Exception("Advanced equipment ID collision");
                        double damageRate=0.94+roll*0.12+tier*0.04+(stars==2?0.12:0);
                        Set(row,"rarity",tier);Set(row,"sellValue",WeaponSaleAssets.Price(tier));
                        Set(row,"weight",Math.Round(F(originalRow,"weight")*(0.92+roll*0.16)*(weapon?1:profile==2?0.8:profile==0?1.12:profile==1?0.95:1.08),2));
                        if(weapon) {
                            for(int e=0;e<5;e++)Set(row,"attackBase"+Damage[e],Math.Clamp((int)Math.Round(I(originalRow,"attackBase"+Damage[e])*damageRate),0,ushort.MaxValue));
                            if(originalRow.ID==baseId&&mode!=0) {
                                int total=(int)Math.Round(Damage.Sum(e=>I(originalRow,"attackBase"+e))*damageRate*1.1);
                                int physical=(int)Math.Round(total*(Mix(seed^53)%100<25?0:0.55));
                                foreach(var element in Damage)Set(row,"attackBase"+element,0);
                                Set(row,"attackBasePhysics",Math.Clamp(physical,0,ushort.MaxValue));Set(row,"attackBase"+Damage[mode],Math.Clamp(total-physical,0,ushort.MaxValue));
                            }
                            double budget=Math.Max(60,Scaling.Sum(k=>F(originalRow,k)))*(0.95+roll*0.1+tier*0.04);
                            double totalWeight=Enumerable.Range(0,5).Where(i=>(mask&(1<<i))!=0).Sum(i=>70d+Mix(seed^(ulong)(200+i))%61);
                            for(int i=0;i<5;i++)Set(row,Scaling[i],(mask&(1<<i))==0?0:Math.Round(budget*(70+Mix(seed^(ulong)(200+i))%61)/totalWeight,2));
                            Set(row,"attackElementCorrectId",CorrectionFirst+mask);
                            foreach(var f in row.Cells.Where(c=>c.Def.InternalName.StartsWith("originEquipWep")).Select(c=>c.Def.InternalName)) {
                                int origin=I(originalRow,f);if(origin>=baseId&&origin<baseId+10000)Set(row,f,id+origin-baseId);else if(origin>=0)throw new Exception("Cross-family advanced reinforcement origin");
                            }
                        } else foreach(var element in new[]{"neutral","slash","blow","thrust","magic","fire","thunder","dark"}) {
                            bool physical=element is "neutral" or "slash" or "blow" or "thrust";
                            double reduction=(1-F(originalRow,element+"DamageCutRate"))*ArmorMultiplier(profile,physical)*(0.95+roll*0.1+tier*0.05);
                            Set(row,element+"DamageCutRate",Math.Clamp(1-reduction,0.1,1));
                        }
                        int j=0;foreach(var slot in slots.Where(k=>I(originalRow,k)<=0).Take(count))Set(row,slot,EffectFirst+choices[j++]*10+tier);
                        p.Rows.Add(row);if(originalRow.ID==baseId)basis=row;
                    }
                    string stats=weapon?$"属性要求：无\\n补正组合：{build}\\n基础攻击力（+0，未质变，未计属性补正）：\\n{string.Join('　',Damage.Select((e,i)=>DamageLabels[i]+" "+I(basis,"attackBase"+e)))}\\n重量：{basis["weight"].Value}\\n质变后伤害构成以装备面板为准。":$"防具类型：{Types[profile]}\\n重量：{basis["weight"].Value}\\n{string.Join('　',new[]{"neutral","magic","fire","thunder","dark"}.Select((e,i)=>(i==0?"物理":DamageLabels[i])+"减伤 "+((1-F(basis,e+"DamageCutRate"))*100).ToString("0.0",CultureInfo.InvariantCulture)+"%"))}";
                    foreach(int fmgId in weapon?new[]{11,21,25}:new[]{12,22,26}) {
                        if(text[fmgId].Entries.Any(e=>e.ID==id))throw new Exception("Advanced affix text collision");
                        string value=fmgId is 11 or 12?name:fmgId is 21 or 22?info:detail[4]+"\\n\\n"+info;
                        text[fmgId].Entries.Add(new FMG.Entry(id,fmgId==25?AffixTextFix.Fit(value.Replace("\\n","\n")):value.Replace("\\n","\n")));
                    }
                    catalog.Add($"{kind}|{id}|{baseId}|{tier}|{name}|{profile}");
                    detailLines.Add(string.Join('\t',new[]{(tag|(uint)id).ToString(),detail[1],name,info,detail[4]+"\\n\\n"+info,stats,detail[6]}));rarities.Add($"{tag|(uint)id}\t{tier}");
                    audit.Add($"{tag|(uint)id}\t{tag|(uint)baseId}\t{kind}\t{tier}\t{profile}\t{(weapon?mask:0)}\t{(weapon?mode:-1)}\t{(weapon?stars:0)}\t{string.Join(',',choices.Select(i=>EffectFirst+i*10+tier))}");
                }
                ordinal++;
            }
            save(table,p);
        }
        foreach(var file in messages.Files){text[file.ID].Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=text[file.ID].Write();}
        File.WriteAllLines(Path.Combine(output,"affix-v2-catalog.txt"),catalog);File.WriteAllLines(Path.Combine(output,"affix-v2-details.tsv"),detailLines);File.WriteAllLines(Path.Combine(output,"affix-v2-rarities.tsv"),rarities);File.WriteAllLines(Path.Combine(output,"affix-v2-audit.tsv"),audit);
        Verify(read,output);
        Console.WriteLine($"Built {catalog.Count} advanced variants and 31 scaling correction masks");
    }
    public static void Verify(Func<string,PARAM> read,string output) {
        var weapons=read("EquipParamWeapon").Rows.ToDictionary(r=>r.ID);
        var armor=read("EquipParamProtector").Rows.ToDictionary(r=>r.ID);
        var corrections=read("AttackElementCorrectParam").Rows.ToDictionary(r=>r.ID);
        var effects=read("SpEffectParam").Rows.Select(r=>r.ID).ToHashSet();
        var lines=File.ReadAllLines(Path.Combine(output,"affix-v2-audit.tsv")).Skip(1).Select(l=>l.Split('\t')).ToArray();
        if(lines.Length!=12328)throw new Exception("Advanced variant coverage mismatch");
        var details=File.ReadAllLines(Path.Combine(output,"affix-v2-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        foreach(var line in lines) {
            uint encoded=uint.Parse(line[0]);int id=(int)(encoded&0x0fffffff),baseId=(int)(uint.Parse(line[1])&0x0fffffff);
            int tier=int.Parse(line[3]),profile=int.Parse(line[4]),mask=int.Parse(line[5]);bool weapon=line[2]=="weapon";
            var row=(weapon?weapons:armor)[id];var source=(weapon?weapons:armor)[baseId];
            if(I(row,"rarity")!=tier)throw new Exception("Advanced rarity mismatch");
            if(weapon) {
                if(!corrections.TryGetValue(I(row,"attackElementCorrectId"),out var correction))throw new Exception("Missing advanced scaling correction");
                if(!ScalingCorrectionFix.Neutral(correction,corrections[ScalingCorrectionFix.Template]))throw new Exception("Advanced scaling correction overrides vanilla rates");
                for(int s=0;s<5;s++) {
                    bool enabled=(mask&(1<<s))!=0;
                    if((F(row,Scaling[s])>0)!=enabled)throw new Exception("Advanced displayed scaling mismatch");
                    foreach(var element in Damage)if(I(correction,$"is{Stats[s]}Correct_by{element}")!=(enabled?1:0))throw new Exception("Advanced scaling does not affect damage");
                }
                if(I(row,"swordArtsParamId")!=I(source,"swordArtsParamId"))throw new Exception("Original weapon skill changed");
                int mode=int.Parse(line[6]);
                if(mode>0&&Damage.Sum(e=>I(source,"attackBase"+e))>0&&I(row,"attackBase"+Damage[mode])<=0)throw new Exception("Advanced elemental damage missing");
                int stars=int.Parse(line[7]);if(details[encoded][2].Count(c=>c=='★')!=stars)throw new Exception("Weapon star text mismatch");
            } else {
                if(!details[encoded][2].Contains(Types[profile]))throw new Exception("Armor archetype label mismatch");
                if(profile==0&&new[]{"magic","fire","thunder","dark"}.Any(e=>Math.Abs(F(row,e+"DamageCutRate")-1)>1e-6))throw new Exception("Physical armor has elemental defense");
                if(profile==1&&new[]{"neutral","slash","blow","thrust"}.Any(e=>Math.Abs(F(row,e+"DamageCutRate")-1)>1e-6))throw new Exception("Magical armor has physical defense");
            }
            foreach(var value in line[8].Split(',',StringSplitOptions.RemoveEmptyEntries))if(!effects.Contains(int.Parse(value)))throw new Exception("Missing advanced affix effect");
        }
        foreach(var group in lines.GroupBy(l=>(l[1],l[2]))) {
            int expected=group.Key.Item2=="weapon"?8:16;
            if(group.Count()!=expected||group.Select(l=>(l[3],l[4])).Distinct().Count()!=expected)throw new Exception("Archetype and rarity are not independent");
            if(group.Key.Item2=="armor")for(int tier=0;tier<4;tier++) {
                var rust=armor[(int)(uint.Parse(group.Single(l=>int.Parse(l[3])==tier&&l[4]=="2")[0])&0x0fffffff)];
                var elden=armor[(int)(uint.Parse(group.Single(l=>int.Parse(l[3])==tier&&l[4]=="3")[0])&0x0fffffff)];
                foreach(var element in new[]{"neutral","slash","blow","thrust","magic","fire","thunder","dark"})
                    if(F(elden,element+"DamageCutRate")>F(rust,element+"DamageCutRate"))throw new Exception("Elden armor is weaker than rusted armor");
                if(F(rust,"weight")>F(elden,"weight"))throw new Exception("Rusted armor weight mismatch");
            }
        }
        Console.WriteLine("Verified advanced scaling masks, damage elements, armor types, stars, native skills and all tier/profile combinations");
    }
    public static void Verify(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        PARAM Read(string name){var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/"+(name=="SpEffectParam"?"SpEffect":name)+".xml"));return p;}
        Verify(Read,input);
    }
}
