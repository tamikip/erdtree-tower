using SoulsFormats;

static class FlaskBatchText {
    public static readonly (int,string)[] Chinese=[
        (9500046,"批量提升圣杯瓶恢复量"),
        (9500047,"使用身上可用的圣杯滴露，批量提升红、蓝圣杯瓶恢复量？最高 +12，多余滴露保留，不补充剩余次数。"),
        (9500048,"需要至少 1 枚圣杯滴露。"),
        (9500049,"红、蓝圣杯瓶恢复量已批量提升，多余滴露已保留。")];
    static readonly (int,string)[] English=[
        (9500046,"Upgrade flask restoration in bulk"),
        (9500047,"Use carried Sacred Tears to upgrade both flasks up to +12? Excess tears are kept. Remaining charges are not refilled."),
        (9500048,"At least one Sacred Tear is required."),
        (9500049,"Both flasks have been upgraded. Excess Sacred Tears were kept.")];
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging required");
        foreach(var (language,entries) in new[]{("zhocn",Chinese),("engus",English)}) {
            var relative=$"msg/{language}/menu_dlc02.msgbnd.dcx";
            var bundle=BND4.Read(Path.Combine(input,relative));
            var before=bundle.Files.ToDictionary(f=>f.ID,f=>f.Bytes.ToArray());
            var file=bundle.Files.Single(f=>f.ID==33);var table=FMG.Read(file.Bytes);
            var original=table.Entries.ToDictionary(e=>e.ID,e=>e.Text);
            foreach(var (id,text) in entries)table.Entries.Single(e=>e.ID==id).Text=text;
            file.Bytes=table.Write();
            var target=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(target)!);bundle.Write(target);
            var check=BND4.Read(target);
            foreach(var other in check.Files.Where(f=>f.ID!=33))if(!other.Bytes.SequenceEqual(before[other.ID]))throw new Exception("Unrelated text table changed");
            var actual=FMG.Read(check.Files.Single(f=>f.ID==33).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
            var expected=entries.ToDictionary(e=>e.Item1,e=>e.Item2);
            if(actual.Count!=original.Count||original.Any(e=>actual[e.Key]!=expected.GetValueOrDefault(e.Key,e.Value)))throw new Exception("Flask text verification failed");
            Console.WriteLine($"Verified {language}: four flask texts updated, all unrelated entries retained");
        }
    }
}
