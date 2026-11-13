using SoulsFormats;

static class EventHostNames {
    public static int Npc(string[] row)=>95016000+int.Parse(row[4])-45011840;
    static bool Equal(object a,object b)=>System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(a,b);
    public static void Patch(string input,string output,string mapInput=null) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        const string mapPath="map/mapstudio/m45_01_00_00.msb.dcx";
        var rows=EventHostAssets.Rows(input);
        var messages=ItemTextTables.Read(BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx")));
        var names=messages["NpcName"].Entries.ToDictionary(e=>e.ID,e=>e.Text);
        foreach(var row in rows)if(!names.TryGetValue(int.Parse(row[5]),out var text)||text!=row[3])throw new Exception($"Native NPC name mismatch: {row[0]}");
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="NpcParam");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml");
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        var npc=PARAM.Read(file.Bytes);npc.ApplyParamdef(def);var template=npc.Rows.Single(r=>r.ID==523200079);
        var own=rows.Select(Npc).ToHashSet();
        foreach(var row in rows) {
            var basis=row[0]=="Dice"?original.Rows.Single(r=>r.ID==523090010):template;
            var named=new PARAM.Row(basis){ID=Npc(row),Name="Tower event host "+row[0]};
            if(row[0]=="Dice")named["teamType"].Value=Convert.ChangeType(26,basis["teamType"].Value.GetType());
            named["nameId"].Value=int.Parse(row[5]);
            int index=npc.Rows.FindIndex(r=>r.ID==named.ID);
            if(index>=0) {
                if(npc.Rows[index].Name!=named.Name)throw new Exception($"NPC name parameter collision: {named.ID}");
                npc.Rows[index]=named;
            }else npc.Rows.Add(named);
        }
        npc.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=npc.Write();
        var map=MSBE.Read(mapInput??Path.Combine(input,mapPath));var beforeMap=map.Write();
        var previous=new Dictionary<uint,int>();
        foreach(var row in rows) {
            var part=map.Parts.Enemies.Single(p=>p.EntityID==uint.Parse(row[1]));
            if(part.CharaInitID!=int.Parse(row[2])||(part.NPCParamID!=523200079&&part.NPCParamID!=Npc(row)))throw new Exception("Unexpected event host parameter");
            previous[part.EntityID]=part.NPCParamID;part.NPCParamID=Npc(row);
        }
        Directory.CreateDirectory(Path.Combine(output,"map/mapstudio"));
        map.Write(Path.Combine(output,mapPath));
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),reg);
        File.Copy(Path.Combine(input,"event-hosts.txt"),Path.Combine(output,"event-hosts.txt"),true);
        var checkReg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        foreach(var f in reg.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(checkReg.Files.Single(c=>c.Name==f.Name).Bytes))throw new Exception("Unrelated param table changed");
        var check=PARAM.Read(checkReg.Files.Single(f=>f.Name==file.Name).Bytes);check.ApplyParamdef(def);var actual=check.Rows.ToDictionary(r=>r.ID);
        if(check.Rows.Count!=original.Rows.Count(r=>!own.Contains(r.ID))+rows.Length)throw new Exception("Unexpected NPC parameter count");
        foreach(var row in original.Rows.Where(r=>!own.Contains(r.ID)))foreach(var cell in row.Cells)
            if(!Equal(cell.Value,actual[row.ID][cell.Def.InternalName].Value))throw new Exception($"Unrelated NPC changed: {row.ID}");
        foreach(var row in rows) {
            var entry=actual[Npc(row)];
            var basis=row[0]=="Dice"?original.Rows.Single(r=>r.ID==523090010):template;
            foreach(var cell in basis.Cells)if(!Equal(entry[cell.Def.InternalName].Value,cell.Def.InternalName=="nameId"?(object)int.Parse(row[5]):row[0]=="Dice"&&cell.Def.InternalName=="teamType"?Convert.ChangeType(26,cell.Value.GetType()):cell.Value))throw new Exception("Named host differs from its template");
            Console.WriteLine($"{row[0]}: {row[3]} (name={row[5]}, npc={entry.ID})");
        }
        var restored=MSBE.Read(Path.Combine(output,mapPath));
        foreach(var row in rows) {
            var part=restored.Parts.Enemies.Single(p=>p.EntityID==uint.Parse(row[1]));
            if(part.NPCParamID!=Npc(row))throw new Exception("Map name binding mismatch");
            part.NPCParamID=previous[part.EntityID];
        }
        if(!beforeMap.SequenceEqual(restored.Write()))throw new Exception("NPC naming changed unrelated map data");
        Console.WriteLine("Verified nine native names; shared NPCs, behavior parameters and unrelated resources preserved");
    }
}
