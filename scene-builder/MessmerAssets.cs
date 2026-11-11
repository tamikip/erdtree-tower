using SoulsFormats;

static class MessmerAssets {
    public static void Patch(string input,string output) {
        var part=BossAssets.Parts(input).Single(p=>p.Index==157);
        if(part.Model!=5130||part.Source!=51300099||part.Think!=51300000)throw new Exception("Messmer must start with phase-one parameters");
        var before=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        BossAssets.Append(input,output,157);
        var path=Path.Combine(output,"regulation.bin");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml");
        PARAM Read(byte[] bytes) {var p=PARAM.Read(bytes);p.ApplyParamdef(def);return p;}
        var npcs=Read(file.Bytes);var tiers=TowerScalingAssets.Tiers(input)[part.Act];
        foreach(bool elite in new[]{false,true})TowerScalingAssets.Apply(npcs.Rows.Single(r=>r.ID==BossAssets.Npc(part,elite)),tiers[elite?1:2]);
        file.Bytes=npcs.Write();SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var other in before.Files.Where(f=>f.Name!=file.Name))if(!other.Bytes.SequenceEqual(check.Files.Single(f=>f.Name==other.Name).Bytes))throw new Exception("Unrelated parameter changed: "+other.Name);
        var original=Read(before.Files.Single(f=>f.Name==file.Name).Bytes);var actual=Read(check.Files.Single(f=>f.Name==file.Name).Bytes);
        if(original.Rows.Count!=actual.Rows.Count)throw new Exception("Unexpected NPC count");
        var source=actual.Rows.Single(r=>r.ID==51300099);
        foreach(var row in actual.Rows) {
            var old=original.Rows.Single(r=>r.ID==row.ID);bool ours=row.ID==BossAssets.Npc(part,false)||row.ID==BossAssets.Npc(part,true);
            foreach(var cell in row.Cells) {
                var name=cell.Def.InternalName;
                if(!ours&&!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,old[name].Value))throw new Exception($"Unrelated NPC changed: {row.ID}/{name}");
                if(ours&&(name.StartsWith("modelDispMask")||name=="spEffectID31")&&!Equals(cell.Value,source[name].Value))throw new Exception("Phase-one appearance/effect mismatch");
                if(ours&&name is "hp" or "getSoul"&&!Equals(cell.Value,old[name].Value))throw new Exception("Tower health/reward changed");
            }
        }
        Console.WriteLine("Verified Messmer phase-one NPCs, appearance and initial effect; tower health/rewards retained; unrelated parameters unchanged");
    }
}
