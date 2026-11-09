using SoulsFormats;

static class PhysickAssets {
    static readonly Dictionary<string,int> Fields=new() {
        ["replaceItemId"]=-1,["isSuppleItem"]=0,["isFullSuppleItem"]=0,["autoReplenishType"]=0
    };
    public static void Apply(PARAM goods) {
        foreach(var id in new[]{250,251}) {
            var row=goods.Rows.Single(r=>r.ID==id);
            foreach(var (key,value) in Fields)Set(row,key,value);
            Set(row,"isConsume",id==250?1:0);
            Set(row,"maxNum",id==250?999:1);
            Set(row,"maxRepositoryNum",id==250?999:1);
        }
    }
    static void Set(PARAM.Row row,string field,int value)=>row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
    public static void Verify(PARAM goods) {
        foreach(var id in new[]{250,251}) {
            var row=goods.Rows.Single(r=>r.ID==id);
            foreach(var (key,value) in Fields)
                if(Convert.ToInt32(row[key].Value)!=value)throw new Exception($"Physick {id}: {key}");
            if(Convert.ToInt32(row["maxNum"].Value)!=(id==250?999:1)||
                Convert.ToInt32(row["maxRepositoryNum"].Value)!=(id==250?999:1)||
                Convert.ToInt32(row["isConsume"].Value)!=(id==250?1:0))throw new Exception("Physick stack lifecycle mismatch");
        }
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var names=ItemTextTables.Read(BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx")))["GoodsName"];
        if(names.Entries.Single(e=>e.ID==9500).Text!="龟裂壶")throw new Exception("Starting cracked pot ID mismatch");
        Console.WriteLine("Verified starting item: 9500 = 龟裂壶");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="EquipParamGoods");
        var def=PARAMDEF.XmlDeserialize(Path.Combine(Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs","EquipParamGoods.xml"));
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var goods=PARAM.Read(file.Bytes);goods.ApplyParamdef(def);Apply(goods);Verify(goods);
        file.Bytes=goods.Write();
        Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files.Where(f=>f!=file))
            if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);Verify(actual);
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        var changed=Fields.Keys.Concat(new[]{"isConsume","maxNum","maxRepositoryNum"}).ToHashSet();
        foreach(var row in original.Rows)foreach(var cell in row.Cells) {
            if(row.ID is 250 or 251&&changed.Contains(cell.Def.InternalName))continue;
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))
                throw new Exception($"Unrelated goods changed: {row.ID} {cell.Def.InternalName}");
        }
        const string menuPath="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,menuPath));
        var textFile=menu.Files.Single(f=>f.ID==33);var text=FMG.Read(textFile.Bytes);
        foreach(var (id,label) in new[]{(9500000,"补给：2 红 1 蓝"),(9500044,"补给：1 红 2 蓝"),(9500106,"补给：3 红"),(9500107,"补给：3 蓝")})
            text.Entries.Single(e=>e.ID==id).Text=label;
        textFile.Bytes=text.Write();
        var menuOutput=Path.Combine(output,menuPath);Directory.CreateDirectory(Path.GetDirectoryName(menuOutput)!);menu.Write(menuOutput);
        var verified=BND4.Read(menuOutput);
        foreach(var f in menu.Files)if(!f.Bytes.SequenceEqual(verified.Files.Single(c=>c.ID==f.ID).Bytes))throw new Exception("Physick text roundtrip failed");
        Console.WriteLine("Verified stackable consumable Physick, no automatic refill; only bottle rows and four camp labels changed.");
    }
}
