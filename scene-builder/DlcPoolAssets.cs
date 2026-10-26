using SoulsFormats;

static class DlcPoolAssets {
    public static void Verify(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var parameters=new Dictionary<string,PARAM>();
        PARAM Read(string name) {
            if(parameters.TryGetValue(name,out var existing))return existing;
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{(name=="Magic"?"MagicParam":name=="SpEffectParam"?"SpEffect":name)}.xml"));
            parameters[name]=p;return p;
        }
        var text=ItemTextTables.Read(BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx")));
        var details=File.ReadAllLines(Path.Combine(input,"reward-details.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>uint.Parse(p[0]));
        var rewards=File.ReadAllLines(Path.Combine(input,"reward-catalog.txt")).Concat(File.ReadAllLines(Path.Combine(input,"crystal-tears.txt"))).Select(l=>l.Split('|')).ToArray();
        foreach(var entry in rewards) {
            var (table,fmg,tag)=entry[0] switch {"weapon"=>("EquipParamWeapon","WeaponName",0u),"armor"=>("EquipParamProtector","ProtectorName",0x10000000u),"talisman"=>("EquipParamAccessory","AccessoryName",0x20000000u),_=>("EquipParamGoods","GoodsName",0x40000000u)};
            int id=int.Parse(entry[1]);var row=Read(table).Rows.Single(r=>r.ID==id);
            if(!text[fmg].Entries.Any(e=>e.ID==id&&EquipmentStyleAssets.SameName(e.Text,entry[2])))throw new Exception($"Reward name missing: {entry[0]}:{id}");
            var detail=details[tag|(uint)id];
            if(entry[0]=="weapon"&&Convert.ToInt32(row["originEquipWep1"].Value)>=0) {
                int cap=Convert.ToInt32(row["originEquipWep16"].Value)<0?10:25;
                for(int level=0;level<=cap;level++)
                    if(!Read("ReinforceParamWeapon").Rows.Any(r=>r.ID==Convert.ToInt32(row["reinforceTypeId"].Value)+level))throw new Exception($"Weapon reinforcement missing: {id}+{level}");
            }
            if(detail.Length!=7||string.IsNullOrWhiteSpace(detail[4])||new FileInfo(Path.Combine(input,"reward-icons",detail[1]+".rgba")).Length!=160*160*4)throw new Exception($"Reward presentation missing: {id}");
            if(entry[0]=="spirit")for(int level=0;level<=10;level++) {
                var ash=Read(table).Rows.Single(r=>r.ID==id+level);
                if(Convert.ToInt32(ash["consumeMP"].Value)!=0||Convert.ToInt32(ash["consumeHP"].Value)!=-1)throw new Exception($"Spirit cost changed: {id+level}");
            }
        }
        foreach(var line in File.ReadAllLines(Path.Combine(input,"shop-catalog.txt")).Concat(File.ReadAllLines(Path.Combine(input,"shop-extras.txt")).Select(l=>string.Join('|',l.Split('|').Skip(1))))) {
            var p=line.Split('|');var kind=int.Parse(p[0]);var id=int.Parse(p[1]);
            var table=kind switch {0=>"EquipParamWeapon",3=>"EquipParamGoods",4=>"EquipParamGem",_=>throw new Exception("Unexpected shop category")};
            if(!Read(table).Rows.Any(r=>r.ID==id))throw new Exception($"Shop parameter missing: {line}");
        }
        RequirementsAssets.Verify(Read);
        var weaponIds=rewards.Where(p=>p[0]=="weapon").Select(p=>int.Parse(p[1])).ToHashSet();
        var expectedEpic=Read("EquipParamWeapon").Rows.Where(r=>weaponIds.Contains(r.ID)&&Convert.ToInt32(r["rarity"].Value)==2).Select(r=>r.ID).ToHashSet();
        if(!expectedEpic.SetEquals(File.ReadAllLines(Path.Combine(input,"epic-weapons.txt")).Select(int.Parse)))throw new Exception("Epic reward pool incomplete");
        AdvancedAffixAssets.Verify(Read,input);
        if(File.Exists(Path.Combine(input,"random-ashes.tsv")))EquipmentStyleAssets.Verify(Read,input);
        Console.WriteLine($"Verified {rewards.Length} reward entries, native/DLC names, descriptions, icons, spirit levels and shop parameter coverage");
    }
}
