using SoulsFormats;

static class GameVersionAssets {
    public static void Rebase(string game,string input,string output) {
        var latest=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        var mod=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        // Tables not listed here must stay byte-identical to the game's regulation; [] allows new rows only.
        var allowed=new Dictionary<string,string[]> {
            ["ActionButtonParam"]=[], ["EquipMtrlSetParam"]=[], ["ItemLotParam_map"]=[], ["ShopLineupParam"]=[], ["SpEffectParam"]=[],
            ["EquipParamProtector"]=[], ["AttackElementCorrectParam"]=[],
            ["AtkParam_Pc"]=[], ["BehaviorParam_PC"]=[], ["SpEffectVfxParam"]=[],
            ["NpcParam"]=["disableRespawn"],
            ["BuddyStoneParam"]=["disableParam_NT","summonedEventFlagId","warnRegionEntityId"],
            ["Bullet"]=["atkId_Bullet"],
            ["EquipParamGoods"]=["isConsume","consumeMP","consumeHP","isSuppleItem","isFullSuppleItem","autoReplenishType"],
            ["EquipParamWeapon"]=["reinforcePrice","materialSetId","properStrength","properAgility","properFaith","properLuck","properMagic","sellValue"],
            ["Magic"]=["requirementIntellect","requirementFaith","requirementLuck"]
        };
        var changed=new HashSet<string>();
        foreach(var file in mod.Files) {
            var name=Path.GetFileNameWithoutExtension(file.Name);
            var target=latest.Files.Single(f=>Path.GetFileName(f.Name)==Path.GetFileName(file.Name));
            if(target.Bytes.SequenceEqual(file.Bytes))continue;
            if(!allowed.TryGetValue(name,out var fields))throw new Exception($"Unreviewed parameter change: {name}");
            var defName=name switch {"ItemLotParam_map"=>"ItemLotParam","Bullet"=>"BulletParam","Magic"=>"MagicParam","SpEffectParam"=>"SpEffect","AtkParam_Pc"=>"AtkParam","BehaviorParam_PC"=>"BehaviorParam","SpEffectVfxParam"=>"SpEffectVfx",_=>name};
            var def=PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{defName}.xml");
            var original=PARAM.Read(target.Bytes);var customized=PARAM.Read(file.Bytes);
            if(original.ParamType!=customized.ParamType||original.ParamdefDataVersion!=customized.ParamdefDataVersion)throw new Exception($"Parameter layout changed: {name}");
            original.ApplyParamdef(def);customized.ApplyParamdef(def);
            var rows=customized.Rows.ToDictionary(r=>r.ID);
            foreach(var row in original.Rows) {
                if(!rows.TryGetValue(row.ID,out var replacement))throw new Exception($"New game row missing from mod: {name}/{row.ID}");
                foreach(var cell in row.Cells) {
                    if(System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,replacement[cell.Def.InternalName].Value))continue;
                    if(!fields.Contains(cell.Def.InternalName))throw new Exception($"Unreviewed cell: {name}/{row.ID}/{cell.Def.InternalName}");
                    if(name=="BuddyStoneParam"&&!new[]{45000100,45010100,45020100}.Contains(row.ID)||name=="Bullet"&&row.ID!=20003309)throw new Exception($"Unreviewed row: {name}/{row.ID}");
                }
            }
            target.Bytes=file.Bytes;changed.Add(name);
        }
        var untouched=allowed.Keys.Where(k=>!changed.Contains(k)).ToArray();
        if(untouched.Length>0)throw new Exception($"Expected customized tables are identical to the game: {string.Join(",",untouched)}");
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,latest);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        if(check.Version!=latest.Version||check.Files.Count!=latest.Files.Count)throw new Exception("Regulation metadata roundtrip failed");
        foreach(var file in latest.Files)if(!file.Bytes.SequenceEqual(check.Files.Single(f=>f.Name==file.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        Console.WriteLine($"Rebased regulation to {check.Version}: {changed.Count} reviewed custom tables, {check.Files.Count-changed.Count} unchanged vanilla tables; all rows, layouts and encrypted roundtrip verified");
    }
}
