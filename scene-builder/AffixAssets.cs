using SoulsFormats;
using System.Globalization;

static class AffixAssets {
    const int WeaponFirst=100000000,ArmorFirst=10000000,EffectFirst=9850000;
    static readonly string[] Tiers=["普通","稀有","史诗","传说"];
    static readonly string[] Names=["活力","灵慧","耐久","复苏","锋锐","专注","坚韧","力量","灵巧","智识","信仰","感应"];
    static ulong Mix(ulong n) {unchecked {n=(n^(n>>30))*0xbf58476d1ce4e5b9UL;n=(n^(n>>27))*0x94d049bb133111ebUL;return n^(n>>31);}}
    static void Set(PARAM.Row r,string k,object v)=>r[k].Value=Convert.ChangeType(v,r[k].Value.GetType(),CultureInfo.InvariantCulture);
    static int I(PARAM.Row r,string k)=>Convert.ToInt32(r[k].Value);
    static string EffectText(int kind,int tier)=>kind switch {
        0=>$"最大生命 +{2+tier}%",1=>$"最大专注 +{3+tier}%",2=>$"最大耐力 +{2+tier}%",3=>$"耐力恢复速度 +{1+tier}",
        4=>$"各类攻击伤害 +{2+tier}%",5=>$"魔法与祷告专注消耗 -{2+tier}%",6=>$"物理承伤 -{1+tier*0.5:0.#}%",
        _=>$"{new[]{"力量","灵巧","智力","信仰","感应"}[kind-7]} +{1+tier}"
    };
    public static string DescribeEffect(int kind,int tier)=>EffectText(kind,tier);
    static void ApplyEffect(PARAM.Row r,int kind,int tier) {
        Set(r,"artsConsumptionRate",1f);
        if(kind<3)Set(r,new[]{"maxHpRate","maxMpRate","maxStaminaRate"}[kind],1f+(kind==1?3+tier:2+tier)/100f);
        else if(kind==3)Set(r,"staminaRecoverChangeSpeed",1+tier);
        else if(kind==4)foreach(var e in new[]{"physics","magic","fire","thunder","dark"})Set(r,e+"AttackPowerRate",1f+(2+tier)/100f);
        else if(kind==5)foreach(var k in new[]{"magicConsumptionRate","miracleConsumptionRate"})Set(r,k,1f-(2+tier)/100f);
        else if(kind==6)foreach(var k in new[]{"slash","blow","thrust","neutral"})Set(r,k+"DamageCutRate",1f-(1+tier*0.5f)/100f);
        else Set(r,new[]{"addStrengthStatus","addDexterityStatus","addMagicStatus","addFaithStatus","addLuckStatus"}[kind-7],1+tier);
    }
    public static void Build(Func<string,PARAM> read,Action<string,PARAM> save,BND4 messages,string input,string output) {
        var effects=read("SpEffectParam");
        if(effects.Rows.Any(r=>r.ID>=EffectFirst&&r.ID<EffectFirst+120))throw new Exception("Affix effect ID collision");
        for(int kind=0;kind<Names.Length;kind++)for(int tier=0;tier<4;tier++) {
            var row=new PARAM.Row(effects.Rows.Single(r=>r.ID==9600011)){ID=EffectFirst+kind*10+tier,Name=$"Tower affix {kind}:{tier}"};
            ApplyEffect(row,kind,tier);effects.Rows.Add(row);
        }
        save("SpEffectParam",effects);
        var texts=messages.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes));
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var catalog=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0] is "weapon" or "armor").ToList();
        var manifest=new List<string>();var detailLines=new List<string>();var rarityLines=new List<string>();
        foreach(var kind in new[]{"weapon","armor"}) {
            bool weapon=kind=="weapon";uint tag=weapon?0u:0x10000000u;
            string param=weapon?"EquipParamWeapon":"EquipParamProtector";
            var p=read(param);var originals=p.Rows.ToArray();var occupied=originals.Select(r=>r.ID).ToHashSet();
            int ordinal=0;
            foreach(var entry in catalog.Where(e=>e[0]==kind).OrderBy(e=>int.Parse(e[1]))) {
                int baseId=int.Parse(entry[1]);var source=originals.Single(r=>r.ID==baseId);
                var family=weapon?originals.Where(r=>r.ID>=baseId&&r.ID<baseId+10000).ToArray():[source];
                var detail=details[tag|(uint)baseId];
                for(int tier=0;tier<4;tier++) {
                    int id=(weapon?WeaponFirst:ArmorFirst)+(ordinal*4+tier)*(weapon?10000:1000);
                    ulong seed=Mix((ulong)baseId^((ulong)tier<<32)^0xa771f1UL);
                    var choices=Enumerable.Range(0,Names.Length).OrderBy(i=>Mix(seed^(ulong)i)).ToArray();
                    var slotNames=weapon?new[]{"residentSpEffectId","residentSpEffectId1","residentSpEffectId2"}:new[]{"residentSpEffectId","residentSpEffectId2","residentSpEffectId3"};
                    int count=Math.Min(tier<2?1:tier,family.Min(r=>slotNames.Count(k=>I(r,k)<=0)));
                    var description=string.Join("\\n",choices.Take(count).Select(i=>EffectText(i,tier)));
                    if(count==0)description="保留全部固有效果";
                    string name=$"{Tiers[tier]}·{entry[2]}";
                    PARAM.Row basis=null;
                    foreach(var original in family) {
                        var row=new PARAM.Row(original){ID=id+original.ID-baseId,Name=$"Tower affix {baseId}:{tier}"};
                        if(!occupied.Add(row.ID))throw new Exception("Affix equipment ID collision");
                        double roll=(Mix(seed^(ulong)baseId)%1001)/1000.0;
                        Set(row,"weight",Math.Round(Convert.ToDouble(original["weight"].Value)*(0.92+roll*0.16),2));
                        Set(row,"rarity",tier);Set(row,"sellValue",WeaponSaleAssets.Price(tier));
                        if(weapon) {
                            foreach(var f in new[]{"attackBasePhysics","attackBaseMagic","attackBaseFire","attackBaseThunder","attackBaseDark"})
                                Set(row,f,Math.Clamp((int)Math.Round(I(original,f)*(0.96+roll*0.08+tier*0.04)),0,ushort.MaxValue));
                            // Seeded by name length, so INT and FAI share a roll; frozen because v1 variants are already in released runs.
                            foreach(var f in new[]{"correctStrength","correctAgility","correctMagic","correctFaith","correctLuck"})
                                Set(row,f,Math.Round(Convert.ToDouble(original[f].Value)*(0.92+(Mix(seed^(ulong)f.Length)%1001)/1000.0*0.16+tier*0.03),2));
                            foreach(var f in row.Cells.Where(c=>c.Def.InternalName.StartsWith("originEquipWep")).Select(c=>c.Def.InternalName)) {
                                int origin=I(original,f);
                                if(origin>=baseId&&origin<baseId+10000)Set(row,f,id+origin-baseId);
                                else if(origin>=0)throw new Exception($"Cross-family reinforcement origin {baseId}:{f}:{origin}");
                            }
                        } else {
                            foreach(var f in new[]{"neutral","slash","blow","thrust","magic","fire","thunder","dark"}.Select(k=>k+"DamageCutRate")) {
                                double mitigation=1-Convert.ToDouble(original[f].Value);
                                Set(row,f,Math.Clamp(1-mitigation*(0.95+roll*0.1+tier*0.05),0.1,1));
                            }
                        }
                        int j=0;foreach(var slot in slotNames.Where(k=>I(original,k)<=0).Take(count))Set(row,slot,EffectFirst+choices[j++]*10+tier);
                        p.Rows.Add(row);if(original.ID==baseId)basis=row;
                    }
                    foreach(int textTable in weapon?new[]{11,21,25}:new[]{12,22,26}) {
                        string text=textTable is 11 or 12?name:textTable is 21 or 22?description.Replace("\\n","\n"):detail[4].Replace("\\n","\n")+"\n\n"+description.Replace("\\n","\n");
                        if(textTable==25)text=AffixTextFix.Fit(text);
                        if(texts[textTable].Entries.Any(e=>e.ID==id))throw new Exception("Affix FMG collision");
                        texts[textTable].Entries.Add(new FMG.Entry(id,text));
                    }
                    string stats=weapon?$"属性要求：无（保留属性补正）\\n基础攻击力（+0，未计属性补正）：\\n物理 {I(basis,"attackBasePhysics")}　魔力 {I(basis,"attackBaseMagic")}　火 {I(basis,"attackBaseFire")}　雷 {I(basis,"attackBaseThunder")}　圣 {I(basis,"attackBaseDark")}\\n重量：{basis["weight"].Value}":$"重量：{basis["weight"].Value}\\n减伤与重量已随机调整；实际防御以装备菜单为准。";
                    manifest.Add($"{kind}|{id}|{baseId}|{tier}|{name}");
                    detailLines.Add(string.Join('\t',new[]{(tag|(uint)id).ToString(),detail[1],name,description,detail[4]+"\\n\\n"+description,stats,detail[6]}));
                    rarityLines.Add($"{tag|(uint)id}\t{tier}");
                }
                ordinal++;
            }
            save(param,p);
        }
        foreach(var file in messages.Files) {texts[file.ID].Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=texts[file.ID].Write();}
        File.WriteAllLines(Path.Combine(output,"affix-catalog.txt"),manifest);
        File.WriteAllLines(Path.Combine(output,"affix-details.tsv"),detailLines);
        File.WriteAllLines(Path.Combine(output,"affix-rarities.tsv"),rarityLines);
        Console.WriteLine($"Built {manifest.Count} affixed equipment variants and {Names.Length*4} effects");
    }
    public static void Patch(string input,string output,bool advanced=false) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging directory required");
        Directory.CreateDirectory(output);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var originals=new Dictionary<string,PARAM>();var generated=new Dictionary<string,PARAM>();
        PARAM Read(string name) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/"+(name=="SpEffectParam"?"SpEffect":name)+".xml");
            if(p.ParamType!=def.ParamType||p.DetectedSize!=def.GetRowSize())throw new Exception("Affix paramdef mismatch");
            p.ApplyParamdef(def);originals[name]=p;return p;
        }
        void Save(string name,PARAM p) {p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));generated[name]=p;reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes=p.Write();}
        var messages=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));
        var originalText=messages.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.Select(e=>(e.ID,e.Text)).ToArray());
        if(advanced)AdvancedAffixAssets.Build(Read,Save,messages,input,output);else Build(Read,Save,messages,input,output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        if(new FileInfo(path).Length>4799*1024)throw new Exception("Affixed regulation exceeds the game's regulation size limit");
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        var effectIds=generated["SpEffectParam"].Rows.Select(e=>e.ID).ToHashSet();
        foreach(var f in check.Files) {
            var name=Path.GetFileNameWithoutExtension(f.Name);
            if(!generated.ContainsKey(name)) {if(!before[f.Name].SequenceEqual(f.Bytes))throw new Exception("Unrelated parameter changed");continue;}
            if(!reg.Files.Single(x=>x.Name==f.Name).Bytes.SequenceEqual(f.Bytes))throw new Exception("Affix encryption roundtrip mismatch");
            var actual=PARAM.Read(f.Bytes);actual.ApplyParamdef(generated[name].AppliedParamdef);
            var old=PARAM.Read(before[f.Name]);old.ApplyParamdef(actual.AppliedParamdef);
            var rows=actual.Rows.ToDictionary(r=>r.ID);
            foreach(var r in old.Rows)foreach(var c in r.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,rows[r.ID][c.Def.InternalName].Value))throw new Exception("Original equipment or effect changed");
            if(name=="EquipParamWeapon")foreach(var r in actual.Rows.Where(r=>r.ID>=WeaponFirst)) {
                foreach(var c in r.Cells.Where(c=>c.Def.InternalName.StartsWith("originEquipWep")))if(Convert.ToInt32(c.Value)>=0&&!rows.ContainsKey(Convert.ToInt32(c.Value)))throw new Exception("Dangling affix reinforcement origin");
                if(new[]{"properStrength","properAgility","properMagic","properFaith","properLuck"}.Any(k=>I(r,k)!=0))throw new Exception("Affix reintroduced ability requirements");
            }
            if(name is "EquipParamWeapon" or "EquipParamProtector")foreach(var r in actual.Rows.Where(r=>r.Name?.StartsWith("Tower affix ")==true)) {
                var fields=name=="EquipParamWeapon"?new[]{"residentSpEffectId","residentSpEffectId1","residentSpEffectId2"}:new[]{"residentSpEffectId","residentSpEffectId2","residentSpEffectId3"};
                foreach(var k in fields)if(I(r,k)>=EffectFirst&&I(r,k)<EffectFirst+120&&!effectIds.Contains(I(r,k)))throw new Exception($"Dangling new affix effect {r.ID}:{k}:{I(r,k)}");
            }
        }
        var msgDir=Path.Combine(output,"msg/zhocn");Directory.CreateDirectory(msgDir);messages.Write(Path.Combine(msgDir,"item_dlc02.msgbnd.dcx"));
        foreach(var file in BND4.Read(Path.Combine(msgDir,"item_dlc02.msgbnd.dcx")).Files) {
            var entries=FMG.Read(file.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
            foreach(var entry in originalText[file.ID])if(!entries.TryGetValue(entry.ID,out var text)||text!=entry.Text)throw new Exception("Original item text changed");
        }
        string Hash(string file)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(file)));
        File.WriteAllText(Path.Combine(output,advanced?"affix-v2-build.json":"affix-build.json"),System.Text.Json.JsonSerializer.Serialize(new {
            sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),sourceItemTextSha256=Hash(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx")),
            builtRegulationSha256=Hash(path),builtItemTextSha256=Hash(Path.Combine(msgDir,"item_dlc02.msgbnd.dcx")),
            variants=File.ReadAllLines(Path.Combine(output,advanced?"affix-v2-catalog.txt":"affix-catalog.txt")).Length,effects=Names.Length*4,
            originalParametersPreserved=true,originalTextPreserved=true,reinforcementOriginsVerified=true,newEffectReferencesVerified=true
        },new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Verified all original rows preserved, origins/effects valid, no requirements, regulation {new FileInfo(path).Length} bytes");
    }
}
