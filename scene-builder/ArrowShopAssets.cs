using SoulsFormats;

static class ArrowShopAssets {
    public static void Build(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam");
        var p=PARAM.Read(file.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/ShopLineupParam.xml"));
        var original=p.Rows.Select(r=>r.ID).ToHashSet();
        for(int i=24;i<26;i++) {
            if(original.Contains(NativeAssets.ShopBase+i))throw new Exception("Arrow shop row already exists");
            int flag=45020700+(i-21)*8;
            if(p.Rows.Any(r=>Convert.ToInt32(r["eventFlag_forStock"].Value)>=flag&&Convert.ToInt32(r["eventFlag_forStock"].Value)<flag+8))throw new Exception("Arrow stock flag collision");
            var row=new PARAM.Row(p.Rows.Single(r=>r.ID==NativeAssets.ShopBase)){ID=NativeAssets.ShopBase+i,Name="Tower arrow stock"};
            void Set(string key,int value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            Set("equipId",i==24?50000000:50010000);Set("equipType",0);Set("value",i==24?20:60);
            Set("sellQuantity",i==24?100:20);Set("setNum",1);Set("eventFlag_forStock",flag);
            p.Rows.Add(row);
        }
        p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=p.Write();Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine("Added two arrow stock rows; regulation roundtrip verified");
    }
    public static void Inspect(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamWeapon").Bytes);
        p.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/EquipParamWeapon.xml"));
        var msg=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));
        var names=FMG.Read(msg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="WeaponName").Bytes);
        foreach(var r in p.Rows.Where(r=>r.ID>=50000000&&r.ID<51000000&&r.ID%10000==0))
            Console.WriteLine($"{r.ID}|{names.Entries.FirstOrDefault(e=>e.ID==r.ID)?.Text}|"+string.Join(" ",r.Cells.Where(c=>new[]{"wepType","attackBasePhysics","attackBaseMagic","attackBaseFire","attackBaseThunder","attackBaseDark"}.Contains(c.Def.InternalName)).Select(c=>$"{c.Def.InternalName}={c.Value}")));
    }
}
