using SoulsFormats;

static class BalanceAssets {
    public const int ExtraEnemyHpPercent=0;
    public static long Scale(long value)=>checked(value*8/5);
    public static long ScaleHp(long value)=>checked(value*(100L+ExtraEnemyHpPercent)/100);
    public static void Apply(PARAM npcs,string root) {
        void Set(int id,int source,long runes,int multiplier=1) {
            var row=npcs.Rows.Single(r=>r.ID==id);
            var original=npcs.Rows.Single(r=>r.ID==source);
            row["hp"].Value=Convert.ChangeType(ScaleHp(Convert.ToInt64(original["hp"].Value)*multiplier),row["hp"].Value.GetType());
            row["getSoul"].Value=Convert.ChangeType(Scale(runes),row["getSoul"].Value.GetType());
        }
        foreach(var line in File.ReadAllLines(Path.Combine(root,"enemy-pools.txt"))) {
            var p=line.Split('|');int model=int.Parse(p[2]);Set(900000000+model*10000,model*10000,int.Parse(p[3]));
        }
        Set(941912000,41912000,0);
        var parts=BossAssets.Parts(root);
        foreach(var p in parts)foreach(bool elite in new[]{false,true})
            Set(BossAssets.Npc(p,elite),p.Source,(elite?p.Runes/3:p.Runes)/parts.Count(m=>m.Encounter==p.Encounter),p.Label.StartsWith("tower_champion_")&&!elite?3:1);
    }
    public static void Patch(string input,string output) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="NpcParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml");
        var npcs=PARAM.Read(file.Bytes);npcs.ApplyParamdef(def);
        var before=PARAM.Read(file.Bytes);before.ApplyParamdef(def);
        Apply(npcs,input);
        var once=npcs.Write();Apply(npcs,input);
        if(!once.SequenceEqual(npcs.Write()))throw new Exception("Balance patch is not idempotent");
        int changed=0;
        foreach(var row in npcs.Rows)foreach(var cell in row.Cells) {
            var old=before.Rows.Single(r=>r.ID==row.ID)[cell.Def.InternalName].Value;
            if(System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(old,cell.Value))continue;
            if(cell.Def.InternalName is not ("hp" or "getSoul")||row.ID<900000000)throw new Exception("Unrelated balance change");
            changed++;
        }
        file.Bytes=once;Directory.CreateDirectory(output);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in reg.Files)if(!f.Bytes.SequenceEqual(check.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Balance roundtrip mismatch");
        Console.WriteLine($"Verified +{ExtraEnemyHpPercent}% HP / +60% runes, {changed} changed cells, idempotence and unchanged other parameters");
    }
}
