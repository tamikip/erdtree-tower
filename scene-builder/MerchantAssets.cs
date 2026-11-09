using SoulsFormats;

static class MerchantAssets {
    const int First=9500000,Base=37,Count=359,Bell=1130392;
    public static void ExpandPots(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"work/SoulsRandomizers/diste/Defs";
        var changed=new HashSet<string>();
        foreach(var eventPath in Directory.GetFiles(Path.Combine(input,"event"),"*.emevd.dcx"))
            foreach(var ins in EMEVD.Read(eventPath).Events.SelectMany(e=>e.Instructions).Where(i=>i.Bank==2003||i.Bank==3&&i.ID==0))
                for(int at=0;at+4<=ins.ArgData.Length;at+=4) {
                    var flag=BitConverter.ToUInt32(ins.ArgData,at);
                    if(flag>=45003968&&flag<45003984)throw new Exception($"New pot stock flag collision: {flag}");
                }
        foreach(var name in new[]{"ShopLineupParam","EquipParamCustomWeapon"}) {
            var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
            var p=PARAM.Read(file.Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,name+".xml")));
            var template=p.Rows.Single(r=>r.ID==First+356);
            for(int i=357;i<Count;i++) {
                if(p.Rows.Any(r=>r.ID==First+i))throw new Exception("Shop capacity rows already exist");
                var row=new PARAM.Row(template){ID=First+i,Name=name=="ShopLineupParam"?"Tower bell stock":"Tower bell ash slot"};
                if(name=="ShopLineupParam") {
                    row["eventFlag_forStock"].Value=Convert.ChangeType(45003968+(i-357)*8,row["eventFlag_forStock"].Value.GetType());
                    row["equipId"].Value=Convert.ChangeType(-1,row["equipId"].Value.GetType());
                }
                p.Rows.Add(row);
            }
            p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=p.Write();changed.Add(file.Name);
        }
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var file in check.Files) {
            if(!changed.Contains(file.Name)) {
                if(!file.Bytes.SequenceEqual(before[file.Name]))throw new Exception("Unrelated param changed");
                continue;
            }
            var name=Path.GetFileNameWithoutExtension(file.Name.Replace('\\','/'));
            var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,name+".xml"));
            var old=PARAM.Read(before[file.Name]);old.ApplyParamdef(def);
            var actual=PARAM.Read(file.Bytes);actual.ApplyParamdef(def);
            if(actual.Rows.Count!=old.Rows.Count+2)throw new Exception("Shop capacity mismatch");
            var rows=actual.Rows.ToDictionary(r=>r.ID);
            foreach(var row in old.Rows)foreach(var cell in row.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,rows[row.ID][cell.Def.InternalName].Value))
                    throw new Exception($"Existing shop field changed: {row.ID}");
        }
        Console.WriteLine("Verified 359 shop/custom weapon rows; only two spare slots added, existing parameters unchanged.");
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        var map=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_02_00_00.msb.dcx"));
        var hunter=map.Parts.Enemies.Single(e=>e.NPCParamID==950000106);
        if(hunter.ModelName!="c3100"||hunter.EntityID<48010000||hunter.EntityID>=48010300)throw new Exception("Shop hunter actor mismatch");
        var events=EMEVD.Read(Path.Combine(input,"event/m45_02_00_00.emevd.dcx"));
        foreach(long id in new long[]{hunter.EntityID,hunter.EntityID+100000})PartyEnemyEvents.Verify(events.Events.Single(e=>e.ID==id),hunter.EntityID);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var originals=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name);
        PARAM Read(string name){var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        void Set(PARAM.Row row,string field,int value)=>row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
        var shops=Read("ShopLineupParam");var custom=Read("EquipParamCustomWeapon");var goods=Read("EquipParamGoods");
        bool Reserved(uint flag)=>flag>=45005200&&flag<45006000||flag>=45015200&&flag<45016000||flag>=45025200&&flag<45026000||flag>=45002904&&flag<45003000||flag>=45003904&&flag<45003984;
        foreach(var eventPath in Directory.GetFiles(Path.Combine(input,"event"),"*.emevd.dcx")) {
            foreach(var ins in EMEVD.Read(eventPath).Events.SelectMany(e=>e.Instructions).Where(i=>i.Bank==2003||i.Bank==3&&i.ID==0))for(int at=0;at+4<=ins.ArgData.Length;at+=4)
                if(Reserved(BitConverter.ToUInt32(ins.ArgData,at)))throw new Exception($"Shop stock flag {BitConverter.ToUInt32(ins.ArgData,at)} collides with {ins.Bank}[{ins.ID}] in {eventPath}");
        }
        foreach(var row in shops.Rows.Where(r=>r.ID<First+Base||r.ID>=First+Count))if(Reserved(unchecked((uint)Convert.ToInt64(row["eventFlag_forStock"].Value))))throw new Exception("Shop stock flag collision");
        for(int i=Base;i<Count;i++) {
            var row=shops.Rows.SingleOrDefault(r=>r.ID==First+i);
            if(row!=null&&row.Name!="Tower bell stock")throw new Exception("Shop slot collision");
            if(row==null){row=new PARAM.Row(shops.Rows.Single(r=>r.ID==First)){ID=First+i,Name="Tower bell stock"};shops.Rows.Add(row);}
            int slot=i-Base;int flag=slot<100?45005200+slot*8:slot<200?45015200+(slot-100)*8:slot<300?45025200+(slot-200)*8:slot<312?45002904+(slot-300)*8:45003904+(slot-312)*8;
            Set(row,"equipId",-1);Set(row,"sellQuantity",1);Set(row,"eventFlag_forStock",flag);
            var weapon=custom.Rows.SingleOrDefault(r=>r.ID==First+i);
            if(weapon!=null&&weapon.Name!="Tower bell ash slot")throw new Exception("Ash slot collision");
            if(weapon==null){weapon=new PARAM.Row(custom.Rows.Single(r=>r.ID==First)){ID=First+i,Name="Tower bell ash slot"};custom.Rows.Add(weapon);}
        }
        var item=BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx"));
        var names=FMG.Read(item.Files.Single(f=>f.ID==10).Bytes);
        var templateId=names.Entries.First(e=>e.Text?.Contains("铃珠")==true&&goods.Rows.Any(r=>r.ID==e.ID)).ID;
        string[] labels=["武器铃珠","防具铃珠","护符铃珠"];
        if(Bell+2>0x7fffff)throw new Exception("Bell IDs exceed the observed goods grant limit");
        for(int i=0;i<3;i++) {
            var old=goods.Rows.SingleOrDefault(r=>r.ID==Bell+i);
            if(old!=null&&old.Name!="Tower merchant bell")throw new Exception("Bell goods collision");
            if(old!=null)goods.Rows.Remove(old);
            var bell=new PARAM.Row(goods.Rows.Single(r=>r.ID==templateId)){ID=Bell+i,Name="Tower merchant bell"};
            foreach(var field in new[]{"refId_default","sellValue"})Set(bell,field,-1);
            foreach(var field in new[]{"disableParam_NT","goodsType","isDiscard","isDeposit","isDrop"})Set(bell,field,0);
            foreach(var cell in bell.Cells.Where(c=>c.Def.InternalName.StartsWith("enable_")))Set(bell,cell.Def.InternalName,0);
            Set(bell,"maxNum",64);goods.Rows.Add(bell);
            if(Convert.ToInt32(bell["disableParam_NT"].Value)!=0)throw new Exception("Bell goods must be enabled");
            foreach(int table in new[]{10,20,24}) {
                var file=item.Files.Single(f=>f.ID==table);var text=FMG.Read(file.Bytes);text.Entries.RemoveAll(e=>e.ID==Bell+i);
                text.Entries.Add(new(Bell+i,table==10?labels[i]:$"击败商店的铃珠猎人获得。\n本局后续每次进入商店，额外出售 5 件对应物品。\n效果可叠加，联机全队共享；本局结束后失效。"));file.Bytes=text.Write();
            }
        }
        foreach(var (name,p) in new[]{("ShopLineupParam",shops),("EquipParamCustomWeapon",custom),("EquipParamGoods",goods)}) {p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));File_(name).Bytes=p.Write();}
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files) {
            if(!new[]{"ShopLineupParam","EquipParamCustomWeapon","EquipParamGoods"}.Contains(Path.GetFileNameWithoutExtension(f.Name))&&!f.Bytes.SequenceEqual(originals[f.Name]))throw new Exception("Unrelated table changed");
        }
        foreach(var (name,p) in new[]{("ShopLineupParam",shops),("EquipParamCustomWeapon",custom),("EquipParamGoods",goods)}) {
            var restored=PARAM.Read(check.Files.Single(f=>f.Name==File_(name).Name).Bytes);restored.ApplyParamdef(p.AppliedParamdef);
            var before=PARAM.Read(originals[File_(name).Name]);before.ApplyParamdef(p.AppliedParamdef);
            foreach(var row in before.Rows.Where(r=>name=="EquipParamGoods"?r.ID<Bell||r.ID>=Bell+3:r.ID<First+Base||r.ID>=First+Count)) {
                var actual=restored.Rows.Single(r=>r.ID==row.ID);
                foreach(var cell in row.Cells)if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,actual[cell.Def.InternalName].Value))throw new Exception($"Unrelated field changed: {name}/{row.ID}");
            }
            if(restored.Rows.Count!=p.Rows.Count)throw new Exception("Row count mismatch");
        }
        var msg=Path.Combine(output,"msg/zhocn/item_dlc02.msgbnd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(msg)!);item.Write(msg);
        var restoredItem=BND4.Read(msg);foreach(var file in item.Files)if(!file.Bytes.SequenceEqual(restoredItem.Files.Single(f=>f.ID==file.ID).Bytes))throw new Exception("Bell text roundtrip failed");
        Console.WriteLine($"Verified shop hunter {hunter.EntityID}, {Count} shop/ash slots, three bell goods and text; unrelated parameters preserved");
    }
}
