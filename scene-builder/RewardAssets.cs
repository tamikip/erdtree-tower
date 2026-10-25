using SoulsFormats;

static class RewardAssets {
    public static void BuildRarities(string game,string output) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        var ids=File.ReadAllLines(Path.Combine(output,"reward-details.tsv")).Select(l=>uint.Parse(l.Split('\t')[0])).ToHashSet();
        var rows=new SortedDictionary<uint,int>();
        foreach(var (name,tag) in new[]{("EquipParamWeapon",0u),("EquipParamProtector",0x10000000u),("EquipParamAccessory",0x20000000u),("EquipParamGoods",0x40000000u)}) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));
            foreach(var r in p.Rows.Where(r=>ids.Contains(tag|(uint)r.ID)))rows.Add(tag|(uint)r.ID,Convert.ToInt32(r["rarity"].Value));
        }
        if(rows.Count!=ids.Count)throw new Exception("Reward rarity coverage mismatch");
        File.WriteAllLines(Path.Combine(output,"reward-rarities.tsv"),rows.Select(r=>$"{r.Key}\t{r.Value}"));
        Console.WriteLine($"Exported native rarity for {rows.Count} reward items");
    }
    public static void Build(string game,string output,string cache) {
        var bundle=BND4.Read(Path.Combine(cache,"item_dlc02.msgbnd.dcx"));
        var tables=bundle.Files.ToDictionary(f=>Path.GetFileNameWithoutExtension(f.Name),f=>FMG.Read(f.Bytes));
        string Text(string table,int id)=>tables[table].Entries.FirstOrDefault(e=>e.ID==id)?.Text??"";
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        var parameters=new Dictionary<string,PARAM>();
        PARAM Param(string name) {
            if(!parameters.ContainsKey(name)) {
                var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
                p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));parameters[name]=p;
            }
            return parameters[name];
        }
        var items=File.ReadAllLines("outputs/erdtree-tower/scene-assets/reward-catalog.txt").Select(line=> {
            var p=line.Split('|');return (kind:p[0],id:int.Parse(p[1]));
        }).ToList();
        items.AddRange(new[]{4000,4001,4002,2130,2180,2060,1150,1030,1032,2160,2080,2081,3000,3001,3070,2020,4021,4022,1000,1001,1002,1020,1021,1022}.Select(id=>("talisman",id)));
        items.Add(("goods",10020));
        items.AddRange(Enumerable.Range(11000,32).Select(id=>("goods",id)));
        var lines=new List<string>();var requiredIcons=new HashSet<int>();
        string Escape(string s)=>s.Replace("\r","").Replace("\n","\\n").Replace("\t"," ");
        foreach(var (kind,id) in items.Distinct()) {
            var (table,param,tag)=kind switch {"weapon"=>("Weapon","EquipParamWeapon",0u),"armor"=>("Protector","EquipParamProtector",0x10000000u),"talisman"=>("Accessory","EquipParamAccessory",0x20000000u),_=>("Goods","EquipParamGoods",0x40000000u)};
            var row=Param(param).Rows.Single(r=>r.ID==id);
            int Int(string key)=>Convert.ToInt32(row[key].Value);
            int icon=Int(kind=="armor"?"iconIdM":"iconId");requiredIcons.Add(icon);
            string stats="",skill="";
            if(kind=="spirit")stats="召唤消耗：0 专注值、0 生命值。\n装备到快捷栏后，在战斗区域召唤。";
            if(kind=="weapon") {
                stats=$"属性要求：无（保留属性补正）\n基础攻击力（+0，未计属性补正）：\n物理 {Int("attackBasePhysics")}　魔力 {Int("attackBaseMagic")}　火 {Int("attackBaseFire")}　雷 {Int("attackBaseThunder")}　圣 {Int("attackBaseDark")}\n重量：{row["weight"].Value}";
                var art=Param("SwordArtsParam").Rows.Single(r=>r.ID==Int("swordArtsParamId"));var text=Convert.ToInt32(art["textId"].Value);
                skill=Text("ArtsName",text)+"\n"+Text("ArtsCaption",text);
                bool magic=Int("enableMagic")!=0,miracle=Int("enableMiracle")!=0;
                if(magic||miracle)stats+="\n施法能力："+(magic&&miracle?"魔法与祷告":magic?"魔法":"祷告");
            }
            if(string.IsNullOrWhiteSpace(Text(table+"Name",id))||string.IsNullOrWhiteSpace(Text(table+"Caption",id)))throw new Exception($"Missing native reward text {kind}:{id}");
            lines.Add(string.Join('\t',new[]{(tag|(uint)id).ToString(),icon.ToString(),Text(table+"Name",id),Text(table+"Info",id),Text(table+"Caption",id),stats,skill}.Select(Escape)));
        }
        Directory.CreateDirectory(output);File.WriteAllLines(Path.Combine(output,"reward-details.tsv"),lines);
        var atlas=TPF.Read(Path.Combine(cache,"01_common.tpf.dcx"));
        var layout=BND4.Read(Path.Combine(cache,"01_common.sblytbnd.dcx"));
        foreach(var file in layout.Files.Where(f=>f.Name.Contains("SB_Icon")))File.WriteAllBytes(Path.Combine(output,Path.GetFileName(file.Name)),file.Bytes);
        foreach(var texture in atlas.Textures.Where(t=>t.Name.StartsWith("SB_Icon")))File.WriteAllBytes(Path.Combine(output,texture.Name+".dds"),texture.Bytes);
        Console.WriteLine($"Exported {lines.Count} original reward descriptions and source atlases for {requiredIcons.Count} icons");
    }
}
