using SoulsFormats;

static class PatchesEventAssets {
    public static void Patch(string input,string output,string game) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        PARAM Read(string name) {var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        var npc=Read("NpcParam");var think=Read("NpcThinkParam");
        if(!think.Rows.Any(r=>r.ID==523090010))throw new Exception("Patches combat AI missing");
        int index=npc.Rows.FindIndex(r=>r.ID==95016006);if(index<0)throw new Exception("Patches event NPC missing");
        var donor=npc.Rows.Single(r=>r.ID==523090010);
        npc.Rows[index]=new PARAM.Row(donor){ID=95016006,Name=npc.Rows[index].Name};
        npc.Rows[index]["teamType"].Value=Convert.ChangeType(26,donor["teamType"].Value.GetType());
        reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam").Bytes=npc.Write();
        const string mapPath="map/mapstudio/m45_01_00_00.msb.dcx";
        var map=MSBE.Read(Path.Combine(input,mapPath));var beforeMap=map.Write();var part=map.Parts.Enemies.Single(e=>e.EntityID==45011826);
        if(part.NPCParamID!=95016006||part.CharaInitID!=23090)throw new Exception("Unexpected Patches actor");
        int previousThink=part.ThinkParamID;part.ThinkParamID=523090010;
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));map.Write(Path.Combine(output,mapPath));
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),reg);
        var checkedMap=MSBE.Read(Path.Combine(output,mapPath));var actor=checkedMap.Parts.Enemies.Single(e=>e.EntityID==45011826);
        if(actor.ThinkParamID!=523090010)throw new Exception("Patches AI roundtrip mismatch");
        actor.ThinkParamID=previousThink;if(!checkedMap.Write().SequenceEqual(beforeMap))throw new Exception("Unrelated map data changed");
        var checkedReg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        foreach(var f in checkedReg.Files) {
            if(Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))!="NpcParam") {if(!f.Bytes.SequenceEqual(before[f.Name]))throw new Exception("Unrelated parameter table changed");continue;}
            var p=PARAM.Read(f.Bytes);p.ApplyParamdef(npc.AppliedParamdef);var original=PARAM.Read(before[f.Name]);original.ApplyParamdef(npc.AppliedParamdef);
            var rows=p.Rows.ToDictionary(r=>r.ID);if(rows.Count!=original.Rows.Count)throw new Exception("NPC count changed");
            foreach(var old in original.Rows)foreach(var c in (old.ID==95016006?donor:old).Cells)
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(old.ID==95016006&&c.Def.InternalName=="teamType"?Convert.ChangeType(26,c.Value.GetType()):c.Value,rows[old.ID][c.Def.InternalName].Value))throw new Exception($"NPC mismatch: {old.ID}:{c.Def.InternalName}");
        }
        Console.WriteLine($"Patches uses combat NPC 523090010 and AI 523090010 (was {previousThink}); other NPCs, maps and spell parameters preserved");
    }
    public static void Inspect(string input) {
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        foreach(var name in new[]{"NpcParam","NpcThinkParam"}) {
            var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));
            foreach(var r in p.Rows.Where(r=>r.ID/1000==523090||r.ID==95016006))Console.WriteLine($"{name}:{r.ID} "+string.Join(" ",r.Cells.Where(c=>new[]{"hp","teamType","nameId","logicId","battleGoalID"}.Contains(c.Def.InternalName)).Select(c=>$"{c.Def.InternalName}={c.Value}")));
        }
        var part=MSBE.Read(Path.Combine(input,"map/mapstudio/m45_01_00_00.msb.dcx")).Parts.Enemies.Single(e=>e.EntityID==45011826);
        Console.WriteLine($"Patches entity: npc={part.NPCParamID} think={part.ThinkParamID} character={part.CharaInitID}");
    }
}
