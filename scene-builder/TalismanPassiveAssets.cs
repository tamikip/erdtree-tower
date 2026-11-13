using SoulsFormats;

static class TalismanPassiveAssets {
    static string English(int kind,int tier)=>new[]{
        $"Max HP +{2+tier}%",$"Max FP +{3+tier}%",$"Max stamina +{2+tier}%",$"Stamina recovery +{1+tier}",
        $"Attack damage +{2+tier}%",$"Spell FP cost -{2+tier}%",$"Physical damage taken -{1+tier*0.5:0.#}%",
        $"Skill damage +{6+2*tier}%",$"Critical damage +{8+3*tier}%",$"Poise damage taken -{5+2*tier}%",$"Runes gained +{5+3*tier}%",
        $"Elemental damage taken -{2+tier}%",$"Charged heavy attack damage +{6+2*tier}%",$"Jump attack damage +{6+2*tier}%",$"Guard counter damage +{8+2*tier}%",
        $"Bow and crossbow damage +{5+2*tier}%",$"Sorcery damage +{4+2*tier}%",$"Incantation damage +{4+2*tier}%",$"Guard stamina cost -{8+3*tier}%",
        $"Action stamina cost -{3+tier}%",$"Flask recovery +{4+2*tier}%"}[kind];
    public static void Build(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output)||File.Exists(Path.Combine(input,"talisman-passives.tsv")))throw new Exception("Use separate staging with pre-passive inputs");
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name,string def) {var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{def}.xml"));return p;}
        var accessories=Read("EquipParamAccessory","EquipParamAccessory");var effects=Read("SpEffectParam","SpEffect");var originals=accessories.Rows.ToDictionary(r=>r.ID);
        var occupied=originals.Keys.ToHashSet();var catalog=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="talisman").OrderBy(p=>int.Parse(p[1])).ToArray();
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var variants=new List<(int id,int basis,int tier,int[] kinds)>();var rows=new List<string>();var lines=new List<string>();var rarities=new List<string>();
        int next=1100000;
        foreach(var p in catalog) {
            int basis=int.Parse(p[1]);var source=originals[basis];
            for(int tier=0;tier<4;tier++) {
                while(occupied.Contains(next))next++;int id=next++;occupied.Add(id);
                var row=new PARAM.Row(source){ID=id,Name=$"Tower passive talisman {basis} tier {tier}"};
                row["rarity"].Value=Convert.ChangeType(tier,row["rarity"].Value.GetType());accessories.Rows.Add(row);
                int count=tier<2?1:tier;
                var kinds=Enumerable.Range(0,AffixAssets.Kinds).OrderBy(k=>AffixAssets.Mix((ulong)basis^((ulong)tier<<32)^(ulong)k^0x7a115aUL)).Take(count).ToArray();
                foreach(int k in kinds)if(!effects.Rows.Any(r=>r.ID==AffixAssets.EffectFirst+k*10+tier))throw new Exception("Passive effect missing");
                string name=$"{new[]{"普通","稀有","史诗","传说"}[tier]}·{p[2]}";
                rows.Add($"{id}\t{basis}\t{tier}\t{string.Join(',',kinds.Select(k=>AffixAssets.EffectFirst+k*10+tier))}\t{name}");
                var d=(string[])details[0x20000000u|(uint)basis].Clone();d[0]=(0x20000000u|(uint)id).ToString();d[2]=name;
                string extra=string.Join("\\n",kinds.Select(k=>AffixAssets.DescribeEffect(k,tier)));
                d[3]=(string.IsNullOrWhiteSpace(d[3])?"":d[3]+"\\n")+extra;d[4]+="\\n\\n"+extra;
                lines.Add(string.Join('\t',d));rarities.Add($"{0x20000000u|(uint)id}\t{tier}");variants.Add((id,basis,tier,kinds));
            }
        }
        for(int copy=10;copy<=13;copy++)for(int kind=0;kind<AffixAssets.Kinds;kind++)for(int tier=0;tier<4;tier++) {
            int id=9880000+copy*1000+kind*10+tier;if(effects.Rows.Any(r=>r.ID==id))throw new Exception($"Passive stack ID collision {id}");
            var source=effects.Rows.Single(r=>r.ID==AffixAssets.EffectFirst+kind*10+tier);
            effects.Rows.Add(new PARAM.Row(source){ID=id,Name=$"{source.Name} stack {copy}"});
        }
        accessories.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));effects.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_("EquipParamAccessory").Bytes=accessories.Write();File_("SpEffectParam").Bytes=effects.Write();
        Directory.CreateDirectory(output);SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),reg);
        foreach(var lang in new[]{"zhocn","engus"}) {
            var msg=BND4.Read(Path.Combine(input,$"msg/{lang}/item_dlc02.msgbnd.dcx"));var merged=ItemTextTables.Read(msg);bool en=lang=="engus";
            foreach(var file in msg.Files) {
                var key=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];if(key is not "AccessoryName" and not "AccessoryInfo" and not "AccessoryCaption")continue;
                var fmg=FMG.Read(file.Bytes);var ids=fmg.Entries.Select(e=>e.ID).ToHashSet();var text=merged[key].Entries.ToDictionary(e=>e.ID,e=>e.Text);
                foreach(var v in variants) {
                    if(ids.Contains(v.id))throw new Exception("Talisman text ID collision");
                    string original=text.GetValueOrDefault(v.basis,"");
                    string value=key=="AccessoryName"?(en?$"{new[]{"Common","Rare","Epic","Legendary"}[v.tier]} {original}":$"{new[]{"普通","稀有","史诗","传说"}[v.tier]}·{original}")
                        :original+"\n\n"+string.Join("\n",v.kinds.Select(k=>en?English(k,v.tier):AffixAssets.DescribeEffect(k,v.tier)));
                    fmg.Entries.Add(new FMG.Entry(v.id,value));
                }
                fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=fmg.Write();
            }
            Directory.CreateDirectory(Path.Combine(output,$"msg/{lang}"));msg.Write(Path.Combine(output,$"msg/{lang}/item_dlc02.msgbnd.dcx"));
        }
        File.WriteAllLines(Path.Combine(output,"talisman-passives.tsv"),rows);File.WriteAllLines(Path.Combine(output,"talisman-passive-details.tsv"),lines);File.WriteAllLines(Path.Combine(output,"talisman-passive-rarities.tsv"),rarities);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        foreach(var file in check.Files) {
            string table=Path.GetFileNameWithoutExtension(file.Name.Replace('\\','/'));
            if(table is not "EquipParamAccessory" and not "SpEffectParam"){if(!file.Bytes.SequenceEqual(before[file.Name]))throw new Exception("Unrelated parameter changed");continue;}
            var def=table=="EquipParamAccessory"?accessories.AppliedParamdef:effects.AppliedParamdef;
            var original=PARAM.Read(before[file.Name]);original.ApplyParamdef(def);var actual=PARAM.Read(file.Bytes);actual.ApplyParamdef(def);var now=actual.Rows.ToDictionary(r=>r.ID);
            foreach(var row in original.Rows)foreach(var c in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,now[row.ID][c.Def.InternalName].Value))throw new Exception("Original row changed");
            if(table=="EquipParamAccessory")foreach(var v in variants)foreach(var c in originals[v.basis].Cells) {
                var expected=c.Def.InternalName=="rarity"?Convert.ChangeType(v.tier,c.Value.GetType()):c.Value;
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(expected,now[v.id][c.Def.InternalName].Value))throw new Exception("Talisman intrinsic effect changed");
            }
            else for(int copy=10;copy<=13;copy++)for(int kind=0;kind<AffixAssets.Kinds;kind++)for(int tier=0;tier<4;tier++)foreach(var c in now[AffixAssets.EffectFirst+kind*10+tier].Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,now[9880000+copy*1000+kind*10+tier][c.Def.InternalName].Value))throw new Exception("Stack effect mismatch");
        }
        Console.WriteLine($"Verified {variants.Count} talisman variants from {catalog.Length} originals; intrinsic effects preserved, 1/1/2/3 extra passives, 14 equipment slots supported");
    }
}
