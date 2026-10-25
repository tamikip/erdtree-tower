using SoulsFormats;

static class WeaponSaleAssets {
    public static int Price(int rarity)=>rarity switch {0=>400,1=>800,2=>1500,_=>2500};
    public static void Apply(PARAM weapons) {
        var bases=weapons.Rows.Where(r=>r.ID%10000==0).ToDictionary(r=>r.ID);
        foreach(var row in weapons.Rows.Where(r=>r.ID>=1000000&&r.ID<50000000)) {
            var basis=bases.GetValueOrDefault(row.ID/10000*10000)??row;
            row["sellValue"].Value=Convert.ChangeType(Price(Convert.ToInt32(basis["rarity"].Value)),row["sellValue"].Value.GetType());
        }
    }
    public static void Patch(string input,string output) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamWeapon");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/EquipParamWeapon.xml");
        var before=PARAM.Read(file.Bytes);before.ApplyParamdef(def);
        var weapons=PARAM.Read(file.Bytes);weapons.ApplyParamdef(def);Apply(weapons);
        var once=weapons.Write();Apply(weapons);
        if(!once.SequenceEqual(weapons.Write()))throw new Exception("Weapon sale patch is not idempotent");
        var originals=before.Rows.ToDictionary(r=>r.ID);int changed=0;
        foreach(var row in weapons.Rows)foreach(var cell in row.Cells) {
            var old=originals[row.ID][cell.Def.InternalName].Value;
            if(System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(old,cell.Value))continue;
            if(cell.Def.InternalName!="sellValue"||row.ID<1000000||row.ID>=50000000)throw new Exception("Unrelated sale parameter changed");
            changed++;
        }
        foreach(var rarity in new[]{0,1,2,3}) {
            if(Price(rarity)!=new[]{400,800,1500,2500}[rarity])throw new Exception("Sale price mismatch");
            if(!weapons.Rows.Any(r=>r.ID>=1000000&&r.ID<50000000&&r.ID%10000==0&&Convert.ToInt32(r["rarity"].Value)==rarity&&Convert.ToInt32(r["sellValue"].Value)==Price(rarity)))throw new Exception("Missing sale rarity coverage");
        }
        file.Bytes=once;Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Sale roundtrip mismatch");
        Console.WriteLine($"Verified weapon sell prices, {changed} changes, idempotence and unchanged other parameters");
    }
}
