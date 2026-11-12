using SoulsFormats;

static class AffixReinforcementFix {
    static readonly string[] Stats=["correctStrength","correctAgility","correctMagic","correctFaith","correctLuck"];
    static int I(PARAM.Row row,string key)=>Convert.ToInt32(row[key].Value);
    static double F(PARAM.Row row,string key)=>Convert.ToDouble(row[key].Value);
    static bool Same(object a,object b)=>System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(a,b);
    static HashSet<int> Families(string input)=>File.ReadAllLines(Path.Combine(input,"affix-v2-catalog.txt")).Select(l=>l.Split('|')).Where(p=>p[0]=="weapon").Select(p=>int.Parse(p[1])/10000).ToHashSet();
    public static void Verify(Func<string,PARAM> read,string input) {
        var families=Families(input);var groups=read("ReinforceParamWeapon").Rows.GroupBy(r=>r.ID/100*100).ToDictionary(g=>g.Key,g=>g.ToArray());
        foreach(var row in read("EquipParamWeapon").Rows.Where(r=>families.Contains(r.ID/10000))) {
            var levels=groups[I(row,"reinforceTypeId")];
            foreach(var stat in Stats.Where(s=>F(row,s)>0))
                if(levels.Any(r=>F(r,stat+"Rate")<=0))throw new Exception($"Affix scaling suppressed: weapon={row.ID} stat={stat} reinforce={I(row,"reinforceTypeId")}");
        }
    }
    public static void Apply(Func<string,PARAM> read,Action<string,PARAM> save,string input) {
        var weapons=read("EquipParamWeapon");var reinforce=read("ReinforceParamWeapon");var families=Families(input);
        var groups=reinforce.Rows.GroupBy(r=>r.ID/100*100).ToDictionary(g=>g.Key,g=>g.ToArray());
        var targets=weapons.Rows.Where(r=>families.Contains(r.ID/10000)&&groups[I(r,"reinforceTypeId")].Any(level=>Stats.Any(s=>F(r,s)>0&&F(level,s+"Rate")==0))).ToArray();
        foreach(var group in targets.GroupBy(r=>I(r,"reinforceTypeId"))) {
            int source=group.Key,target=20000+source;
            if(source>=20000||target>short.MaxValue)throw new Exception($"Affix reinforce reservation collision: {target}");
            if(groups.TryGetValue(target,out var existing)&&existing.Length!=groups[source].Length)throw new Exception($"Affix reinforce reservation collision: {target}");
            foreach(var original in groups[source]) {
                var row=new PARAM.Row(original){ID=target+original.ID-source,Name="Tower affix scaling"};
                foreach(var stat in Stats)if(F(row,stat+"Rate")==0)row[stat+"Rate"].Value=Convert.ChangeType(1,row[stat+"Rate"].Value.GetType());
                if(existing!=null) {
                    var saved=existing.SingleOrDefault(r=>r.ID==row.ID);
                    if(saved==null||row.Cells.Any(c=>!Same(c.Value,saved[c.Def.InternalName].Value)))throw new Exception($"Affix reinforce reservation collision: {row.ID}");
                } else reinforce.Rows.Add(row);
            }
            foreach(var row in group)row["reinforceTypeId"].Value=Convert.ChangeType(target,row["reinforceTypeId"].Value.GetType());
        }
        reinforce.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));
        save("EquipParamWeapon",weapons);save("ReinforceParamWeapon",reinforce);
        Verify(read,input);
        Console.WriteLine($"Restored nonzero scaling multipliers for {targets.Length} affix rows; {targets.Select(r=>I(r,"reinforceTypeId")).Distinct().Count()} private reinforcement families");
    }
    public static void Patch(string input,string output,bool verifyOnly=false) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        BinderFile File_(string name)=>reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name);
        PARAM Read(string name){var p=PARAM.Read(File_(name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        if(verifyOnly){Verify(Read,input);Console.WriteLine("Verified all advanced weapon scaling at every reinforcement level");return;}
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var originalWeapons=Read("EquipParamWeapon");var originalLevels=Read("ReinforceParamWeapon");
        Apply(Read,(name,p)=>File_(name).Bytes=p.Write(),input);
        var families=Families(input);var actual=Read("EquipParamWeapon").Rows.ToDictionary(r=>r.ID);
        foreach(var row in originalWeapons.Rows)foreach(var cell in row.Cells)
            if(!(families.Contains(row.ID/10000)&&cell.Def.InternalName=="reinforceTypeId")&&!Same(cell.Value,actual[row.ID][cell.Def.InternalName].Value))throw new Exception($"Unrelated weapon field changed: {row.ID} {cell.Def.InternalName}");
        var levels=Read("ReinforceParamWeapon").Rows.ToDictionary(r=>r.ID);
        foreach(var row in originalLevels.Rows)foreach(var cell in row.Cells)
            if(!Same(cell.Value,levels[row.ID][cell.Def.InternalName].Value))throw new Exception("Existing reinforcement changed");
        foreach(var row in levels.Values.Where(r=>!originalLevels.Rows.Any(o=>o.ID==r.ID))) {
            var source=levels[row.ID-20000];
            foreach(var cell in row.Cells) {
                var value=source[cell.Def.InternalName].Value;
                if(Stats.Any(s=>s+"Rate"==cell.Def.InternalName)&&Convert.ToDouble(value)==0)value=Convert.ChangeType(1,value.GetType());
                if(!Same(cell.Value,value))throw new Exception("Private reinforcement changed unrelated field");
            }
        }
        foreach(var f in reg.Files.Where(f=>f!=File_("EquipParamWeapon")&&f!=File_("ReinforceParamWeapon")))if(!f.Bytes.SequenceEqual(before[f.Name]))throw new Exception("Unrelated parameter changed");
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        if(new FileInfo(path).Length>4799*1024)throw new Exception("Regulation size limit exceeded");
        var decoded=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in decoded.Files)if(!f.Bytes.SequenceEqual(reg.Files.Single(r=>r.Name==f.Name).Bytes))throw new Exception("Written parameter mismatch");
        Console.WriteLine("Verified original weapons, original reinforcement, other fields and tables unchanged");
    }
}
