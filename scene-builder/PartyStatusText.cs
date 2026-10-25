using SoulsFormats;

static class PartyStatusText {
    public const string Waiting="爬塔队伍尚未就绪，请等待所有人回到大赐福。\n若长时间无法开始，请退出无缝房间后重新加入，并确认双方补丁版本一致。";
    public static void Add(BND4 menu) {
        var file=menu.Files.Single(f=>f.ID==33);var fmg=FMG.Read(file.Bytes);
        fmg.Entries.RemoveAll(e=>e.ID==9500090);fmg.Entries.Add(new FMG.Entry(9500090,Waiting));file.Bytes=fmg.Write();
    }
    public static void Patch(string input,string output) {
        const string name="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,name));Add(menu);
        Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,name))!);menu.Write(Path.Combine(output,name));
        var check=FMG.Read(BND4.Read(Path.Combine(output,name)).Files.Single(f=>f.ID==33).Bytes);
        if(check.Entries.Single(e=>e.ID==9500090).Text!=Waiting)throw new Exception("Party status text mismatch");
        Console.WriteLine("Verified native party connection dialog");
    }
}
