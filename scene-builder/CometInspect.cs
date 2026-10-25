using SoulsFormats;

static class CometInspect {
    public static void Run(string game,string output) {
        Directory.CreateDirectory(output);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        foreach(var name in new[]{"Magic","EquipParamGoods","Bullet"}) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            var def=name=="Magic"?"MagicParam":name=="Bullet"?"BulletParam":name;
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{def}.xml"));
            foreach(var row in p.Rows.Where(r=>name=="Bullet"?r.ID>=10420000&&r.ID<10420100:r.ID>=4200&&r.ID<4210))
                File.WriteAllText(Path.Combine(output,$"{name}-{row.ID}.txt"),string.Join("\n",row.Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
        }
        var path="/chr/c0000.anibnd.dcx";var target=Path.Combine(output,Path.GetFileName(path));
        if(!File.Exists(target))foreach(var archive in new[]{"Data0","Data1","Data2","Data3"}) {
            using var header=UXM.CryptographyUtility.DecryptRsa(Path.Combine(game,archive+".bhd"),UXM.ArchiveKeys.EldenRingKeys[archive]);
            var bhd=BHD5.Read(header,BHD5.Game.EldenRing);ulong key=0;foreach(var ch in path)key=unchecked(key*0x85+ch);
            var file=bhd.Buckets.SelectMany(b=>b).FirstOrDefault(f=>f.FileNameHash==key);if(file==null)continue;
            using var data=File.OpenRead(Path.Combine(game,archive+".bdt"));File.WriteAllBytes(target,file.ReadFile(data));break;
        }
        var binder=BND4.Read(target);
        File.WriteAllLines(Path.Combine(output,"animation-files.txt"),binder.Files.Select(f=>f.Name));
        foreach(var file in binder.Files.Where(f=>f.Name.EndsWith(".tae")))File.WriteAllBytes(Path.Combine(output,Path.GetFileName(file.Name)),file.Bytes);
        var matches=new List<string>();
        foreach(var file in binder.Files.Where(f=>f.Name.EndsWith(".tae"))) {
            var tae=TAE.Read(file.Bytes);
            foreach(var anim in tae.Animations.Where(a=>Path.GetFileName(file.Name)=="a407.tae"&&a.ID>=45000&&a.ID<45200 || a.Events.Any(e=>new[]{526250,526255,10420000,10420001,10420002}.Any(id=>e.GetParameterBytes(false).AsSpan().IndexOf(BitConverter.GetBytes(id))>=0)))) {
                matches.Add($"{Path.GetFileName(file.Name)} animation={anim.ID}");
                matches.AddRange(anim.Events.Select(e=>$"{e.Type} {e.StartTime:F3}-{e.EndTime:F3} {Convert.ToHexString(e.GetParameterBytes(false))}"));
            }
        }
        File.WriteAllLines(Path.Combine(output,"comet-animations.txt"),matches);
        Console.WriteLine($"Read Comet params and {binder.Files.Count} animation entries");
    }
}
