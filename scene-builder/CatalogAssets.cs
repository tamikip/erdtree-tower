using SoulsFormats;

static class CatalogAssets {
    public static void Build(string game,string output,string cache) {
        var binder=BND4.Read(Path.Combine(cache,"item.msgbnd.dcx"));
        var tables=binder.Files.ToDictionary(f=>Path.GetFileNameWithoutExtension(f.Name),f=>FMG.Read(f.Bytes));
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        PARAM Read(string name) {var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        var rewards=new List<string>();var shop=new List<string>();var extras=new List<string>();
        int potGroup=Convert.ToInt32(Read("EquipParamGoods").Rows.Single(r=>r.ID==300)["potGroupId"].Value);
        foreach(var (param,table) in new[]{("EquipParamWeapon","Weapon"),("EquipParamProtector","Protector"),("EquipParamAccessory","Accessory"),("EquipParamGoods","Goods"),("EquipParamGem","Gem")}) {
            var names=tables[table+"Name"].Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text)).GroupBy(e=>e.ID).ToDictionary(g=>g.Key,g=>g.First().Text);
            var captions=tables[table+"Caption"].Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text)).Select(e=>e.ID).ToHashSet();
            foreach(var row in Read(param).Rows.OrderBy(r=>r.ID)) {
                int I(string field)=>Convert.ToInt32(row[field].Value);
                if(!names.TryGetValue(row.ID,out var name)||name.Contains("ERROR")||name.Contains("[")||!captions.Contains(row.ID)||I(table=="Protector"?"iconIdM":"iconId")<=0)continue;
                string kind=table switch {"Weapon"=>"weapon","Protector"=>"armor","Accessory"=>"talisman",_=>""};
                if(table=="Protector"&&row.ID<40000)continue;
                if(table=="Weapon") {
                    if(row.ID<1000000||row.ID%10000!=0||I("wepType")>=80)continue;
                    var group=I("reinforceShopCategory") switch {2=>"staff",3=>"seal",_=>""};
                    if(group!="")extras.Add($"{group}|0|{row.ID}|2400|1|{name}");
                    else shop.Add($"0|{row.ID}|1800|{name}");
                }
                if(table=="Goods") {
                    kind=I("goodsType") switch {5=>"sorcery",16 or 18=>"incantation",7 or 8 when row.ID%1000==0=>"spirit",_=>""};
                    if(I("isConsume")==1&&I("enable_live")==1) {
                        if(name.Contains("肝脏干")||name.EndsWith("苔药"))extras.Add($"resistance|3|{row.ID}|300|10|{name}");
                        else if(name.StartsWith("腌制")||name.EndsWith("肉块")||name is "煮熟虾子" or "煮熟螃蟹")extras.Add($"food|3|{row.ID}|300|5|{name}");
                        else if(name.EndsWith("壶")&&I("potGroupId")==potGroup)extras.Add($"pot|3|{row.ID}|350|3|{name}");
                        else if(name.Contains("飞刀")||name=="扇形暗器")extras.Add($"knife|3|{row.ID}|60|20|{name}");
                    }
                }
                if(table=="Gem")shop.Add($"4|{row.ID}|1200|{name}");
                if(kind!="")rewards.Add($"{kind}|{row.ID}|{name}");
            }
        }
        Directory.CreateDirectory(output);
        File.WriteAllLines(Path.Combine(output,"reward-catalog.txt"),rewards);
        File.WriteAllLines(Path.Combine(output,"shop-catalog.txt"),shop);
        File.WriteAllLines(Path.Combine(output,"shop-extras.txt"),extras);
        foreach(var group in rewards.GroupBy(l=>l.Split('|')[0]))Console.WriteLine($"{group.Key}: {group.Count()}");
        Console.WriteLine($"Shop weapons/ashes of war: {shop.Count()}, extras: {extras.Count}");
    }
}
