using SoulsFormats;

static class WhetbladeAssets {
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging directory required");
        const string name="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,name));
        var file=menu.Files.Single(f=>f.ID==33);var text=FMG.Read(file.Bytes);
        var before=text.Entries.Where(e=>e.ID!=9500042).Select(e=>(e.ID,e.Text)).ToArray();
        text.Entries.Single(e=>e.ID==9500042).Text="免费更换战灰／质变";
        file.Bytes=text.Write();
        var path=Path.Combine(output,name);Directory.CreateDirectory(Path.GetDirectoryName(path)!);menu.Write(path);
        var restored=BND4.Read(path);
        foreach(var f in menu.Files)if(!f.Bytes.SequenceEqual(restored.Files.Single(r=>r.ID==f.ID).Bytes))throw new Exception("Menu text roundtrip mismatch");
        var actual=FMG.Read(restored.Files.Single(f=>f.ID==33).Bytes).Entries;
        if(!before.SequenceEqual(actual.Where(e=>e.ID!=9500042).Select(e=>(e.ID,e.Text))))throw new Exception("Unrelated menu text changed");
        Console.WriteLine("Verified free smith affinity menu label and preservation of other text");
    }
}
