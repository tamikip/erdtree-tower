using SoulsFormats;

static class ResistanceShopAssets {
    public static void Build(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var medicines=File.ReadAllLines(Path.Combine(input,"shop-extras.txt"))
            .Select(l=>l.Split('|')).Where(p=>p[0]=="resistance").ToArray();
        if(medicines.Length!=11||26+medicines.Length!=NativeAssets.ShopCount)throw new Exception("Medicine stock count mismatch");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam");
        var before=file.Bytes;
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/ShopLineupParam.xml");
        var p=PARAM.Read(before);p.ApplyParamdef(def);
        var goods=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamGoods").Bytes);
        var originals=p.Rows.Select(r=>new PARAM.Row(r)).ToArray();
        var ids=Enumerable.Range(NativeAssets.ShopBase+26,medicines.Length).ToHashSet();
        for(int i=0;i<medicines.Length;i++) {
            int index=26+i,id=NativeAssets.ShopBase+index,flag=NativeAssets.StockFlag(index);
            var medicine=medicines[i];int item=int.Parse(medicine[2]);
            if(!goods.Rows.Any(r=>r.ID==item))throw new Exception($"Medicine item {item} missing");
            if(p.Rows.Any(r=>!ids.Contains(r.ID)&&Convert.ToInt32(r["eventFlag_forStock"].Value)>=flag
                &&Convert.ToInt32(r["eventFlag_forStock"].Value)<flag+8))throw new Exception("Medicine stock flag collision");
            var row=p.Rows.SingleOrDefault(r=>r.ID==id);
            if(row==null){row=new PARAM.Row(p.Rows.Single(r=>r.ID==NativeAssets.ShopBase)){ID=id};p.Rows.Add(row);}
            void Set(string key,int value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            row.Name="Tower resistance medicine: "+medicine[5];
            Set("equipId",item);Set("equipType",3);Set("value",int.Parse(medicine[3]));
            Set("sellQuantity",10);Set("setNum",1);Set("eventFlag_forStock",flag);
        }
        file.Bytes=p.Write();Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        var restored=PARAM.Read(check.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam").Bytes);restored.ApplyParamdef(def);
        restored.Rows.RemoveAll(r=>ids.Contains(r.ID)&&!originals.Any(old=>old.ID==r.ID));
        foreach(var old in originals.Where(r=>ids.Contains(r.ID)))restored.Rows[restored.Rows.FindIndex(r=>r.ID==old.ID)]=new PARAM.Row(old);
        if(!restored.Write().SequenceEqual(before))throw new Exception("Unrelated shop data changed");
        Console.WriteLine("Verified 11 medicine rows, stock flags, item IDs and regulation roundtrip; unrelated data preserved");
    }
}
