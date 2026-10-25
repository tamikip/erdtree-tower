using SoulsFormats;

static class RecoveryText {
    static readonly (int,string)[] StormEntries=[(9500115,"男生女生向前冲"),(9500116,"领取 800 卢恩，跳过挑战"),(9500117,"泥头车跑酷")];
    static readonly (int,string)[] Entries=[(9500106,"补给：3 红＋灵药"),(9500107,"补给：3 蓝＋灵药"),(9500091,"回满生命"),(9500092,"回满专注"),(9500093,"泉水已使用。"),(9500094,"是男人就下一百层"),(9500085,"付费后等待三秒，三只粪金龟会现身。你有六秒时间，每击败一只可得一件宝物。每人最多玩三次。")];
    static readonly (int,string)[] TrialEntries=[
        (9500118,"癫火封印上方的深坑跳跳乐，抵达底部领取传说武器宝箱。\n单人死亡直接结算本局。联机确认后提交全队投票。\n确定参加？"),
        (9500119,"从史东薇尔城正面跑到葛瑞克雾门前，获得两个随机护符增益。\n单人死亡直接结算本局。联机确认后提交全队投票。\n确定参加？"),
        (9500120,"竞技场生存挑战：5 秒后战车出现并追赶玩家，坚持 60 秒领取传说宝箱。\n单人死亡直接结算本局。联机确认后提交全队投票。\n确定参加？"),
        (9500121,"领取 800 卢恩并跳过本次挑战。\n联机确认后提交投票，最终选择由全队投票决定。\n确定选择？")];
    static readonly (int,string)[] ChestEntries=[(9500053,"三个宝箱，三份命运。每箱两成火药、两成守卫、六成宝物。")];
    static readonly (int,string)[] EndEntries=[(9500108,"结束本局"),(9500109,"结束本局并返回大赐福？联机时，全队将结束本局。"),(9500110,"当前无法结束本局。联机时请由房主操作。"),(9500111,"出售武器")];
    public static void Add(BND4 menu,bool chestOnly=false) {
        var file=menu.Files.Single(f=>f.ID==33);var fmg=FMG.Read(file.Bytes);
        foreach(var (id,text) in chestOnly?ChestEntries:Entries.Concat(EndEntries).Concat(StormEntries).Concat(TrialEntries)){fmg.Entries.RemoveAll(e=>e.ID==id);fmg.Entries.Add(new FMG.Entry(id,text));}
        file.Bytes=fmg.Write();
    }
    public static void AddTalismans(BND4 menu,BND4 regulation,BND4 item) {
        PARAM Read(string name) {var p=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        var materials=Read("EquipMtrlSetParam").Rows.ToDictionary(r=>r.ID);
        var names=FMG.Read(item.Files.Single(f=>f.ID==13).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
        var file=menu.Files.Single(f=>f.ID==33);var text=FMG.Read(file.Bytes);
        void Set(int id,string value){text.Entries.RemoveAll(e=>e.ID==id);text.Entries.Add(new FMG.Entry(id,value));}
        Set(9500112,"没有可强化的护符。需要持有低阶护符，且尚未持有其强化目标。");
        Set(9500113,"消耗本次营地强化机会，将所选护符兑换为高阶版本？已装备的护符会在原栏位替换。");
        Set(9500114,"护符强化完成。");
        foreach(var row in Read("ShopLineupParam").Rows.Where(r=>r.ID>=SmithAssets.ShopFirst&&r.ID<SmithAssets.ShopFirst+100)) {
            var material=materials[Convert.ToInt32(row["mtrlId"].Value)];
            Set(9515000+row.ID-SmithAssets.ShopFirst,$"{names[Convert.ToInt32(material["materialId02"].Value)]} → {names[Convert.ToInt32(row["equipId"].Value)]}");
        }
        file.Bytes=text.Write();
    }
    public static void PatchTalismans(string input,string output) {
        const string name="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var menu=BND4.Read(Path.Combine(input,name));
        AddTalismans(menu,SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin")),BND4.Read(Path.Combine(input,"msg/zhocn/item_dlc02.msgbnd.dcx")));
        var path=Path.Combine(output,name);Directory.CreateDirectory(Path.GetDirectoryName(path)!);menu.Write(path);
        var expected=FMG.Read(menu.Files.Single(f=>f.ID==33).Bytes).Entries;
        var actual=FMG.Read(BND4.Read(path).Files.Single(f=>f.ID==33).Bytes).Entries;
        if(expected.Count!=actual.Count||expected.Any(e=>actual.Single(a=>a.ID==e.ID).Text!=e.Text))throw new Exception("Talisman menu text mismatch");
        Console.WriteLine("Verified talisman choice and confirmation text roundtrip");
    }
    public static void Patch(string input,string output,bool chestOnly=false) {
        var entries=chestOnly?ChestEntries:Entries.Concat(EndEntries).Concat(StormEntries).Concat(TrialEntries).ToArray();
        const string name="msg/zhocn/menu_dlc02.msgbnd.dcx";
        var original=BND4.Read(Path.Combine(input,name));
        var menu=BND4.Read(Path.Combine(input,name));Add(menu,chestOnly);
        var path=Path.Combine(output,name);Directory.CreateDirectory(Path.GetDirectoryName(path)!);menu.Write(path);
        var check=FMG.Read(BND4.Read(path).Files.Single(f=>f.ID==33).Bytes);
        foreach(var (id,text) in entries)if(check.Entries.Single(e=>e.ID==id).Text!=text)throw new Exception("Event text mismatch");
        var actual=BND4.Read(path);
        foreach(var file in original.Files.Where(f=>f.ID!=33))if(!file.Bytes.SequenceEqual(actual.Files.Single(f=>f.ID==file.ID).Bytes))throw new Exception("Unrelated message file changed");
        foreach(var entry in FMG.Read(original.Files.Single(f=>f.ID==33).Bytes).Entries.Where(e=>!entries.Any(v=>v.Item1==e.ID)))if(check.Entries.Single(e=>e.ID==entry.ID).Text!=entry.Text)throw new Exception("Unrelated event text changed");
        Console.WriteLine($"Verified {entries.Length} event messages; unrelated messages preserved");
    }
}
