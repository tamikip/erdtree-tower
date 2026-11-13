using SoulsFormats;

static class FrostShackleAssets {
    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        PARAM Read(string name) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{(name=="SpEffectParam"?"SpEffect":name)}.xml"));return p;
        }
        var effects=Read("SpEffectParam");var goods=Read("EquipParamGoods");
        void Put(PARAM p,PARAM.Row row) {
            var old=p.Rows.SingleOrDefault(r=>r.ID==row.ID);
            if(old!=null&&old.Name!=row.Name)throw new Exception($"ID collision: {row.ID}");
            p.Rows.RemoveAll(r=>r.ID==row.ID);p.Rows.Add(row);p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        }
        Put(effects,EmberAssets.FrostShackle(effects));
        Put(goods,new PARAM.Row(goods.Rows.Single(r=>r.ID==9900002)){ID=9900037,Name="Tower ember display FrostShackle"});
        foreach(var (name,p,id) in new[]{("SpEffectParam",effects,9670028),("EquipParamGoods",goods,9900037)}) {
            var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
            var original=PARAM.Read(before[file.Name]);original.ApplyParamdef(p.AppliedParamdef);
            var actual=p.Rows.Where(r=>r.ID!=id).ToDictionary(r=>r.ID);
            if(actual.Count!=original.Rows.Count(r=>r.ID!=id))throw new Exception("Unrelated row count changed");
            foreach(var old in original.Rows.Where(r=>r.ID!=id))foreach(var c in old.Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.Value,actual[old.ID][c.Def.InternalName].Value))throw new Exception($"Unrelated row changed: {name}:{old.ID}");
            file.Bytes=p.Write();
        }
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(x=>x.Name==f.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        foreach(var lang in new[]{"zhocn","engus"}) {
            var relative=$"msg/{lang}/item_dlc02.msgbnd.dcx";var msg=BND4.Read(Path.Combine(input,relative));
            var names=msg.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/')).Split("_dlc")[0]=="GoodsName").ToArray();
            if(names.Length==0)throw new Exception("GoodsName table missing");
            foreach(var table in names) {
                var fmg=FMG.Read(table.Bytes);fmg.Entries.RemoveAll(e=>e.ID==9900037);
                fmg.Entries.Add(new FMG.Entry(9900037,lang=="zhocn"?"余烬：寒霜枷锁":"Ember: Frost Shackles"));table.Bytes=fmg.Write();
            }
            var dest=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(dest)!);msg.Write(dest);
            var roundtrip=BND4.Read(dest);
            foreach(var f in msg.Files)if(!f.Bytes.SequenceEqual(roundtrip.Files.Single(x=>x.ID==f.ID).Bytes))throw new Exception("Text roundtrip mismatch");
        }
        Console.WriteLine("Frost Shackles: effect 9670028, 10 seconds, five outgoing attack rates 0.8; display goods 9900037 reuses frost icon 65002. Existing rows preserved.");
    }
}
