using SoulsFormats;

static class ItemTextTables {
    public static Dictionary<string,FMG> Read(BND4 binder) {
        var result=new Dictionary<string,FMG>();
        foreach(var file in binder.Files.OrderBy(f=>f.ID)) {
            var name=Path.GetFileNameWithoutExtension(file.Name).Split("_dlc")[0];
            if(!result.TryGetValue(name,out var table))result[name]=table=new FMG();
            foreach(var entry in FMG.Read(file.Bytes).Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text))) {
                var old=table.Entries.FindIndex(e=>e.ID==entry.ID);
                if(old<0)table.Entries.Add(entry);else table.Entries[old]=entry;
            }
        }
        return result;
    }
}
