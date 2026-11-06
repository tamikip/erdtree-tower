using SoulsFormats;

static class RecoveryText {
    static readonly (int,string)[] MausoleumEntries=[(9500160,"复制当前右手武器（免费）"),(9500161,"复制当前右手武器，保留强化、质变、战灰和词条。\n原武器不会消耗，每人每次事件限一次。\n确定复制？"),(9500162,"离开灵庙，继续旅程"),(9500163,"武器复制完成，副本已放入背包。"),(9500164,"前往灵庙"),(9500165,"前往龙王平台上的灵庙，免费复制一把武器。\n强化、质变、战灰和词条保留。\n联机确认后提交全队投票。确定前往？"),(9500166,"放弃前往，领取 800 卢恩"),(9500167,"放弃本次灵庙复制，领取 800 卢恩并继续旅程。\n联机确认后提交全队投票。确定领取？")];
    static readonly (int,string)[] PriceEntries=[(9500080,"捕捉粪金龟 · 500 卢恩"),(9500084,"需要 500 卢恩。"),(9500158,"领取 800 卢恩（放弃恢复）")];
    static readonly (int,string)[] StormEntries=[(9500115,"男生女生向前冲"),(9500116,"领取 800 卢恩，跳过挑战"),(9500117,"泥头车跑酷")];
    static readonly (int,string)[] Entries=[(9500106,"补给：3 红＋灵药"),(9500107,"补给：3 蓝＋灵药"),(9500091,"回满生命"),(9500092,"回满专注"),(9500093,"泉水已使用。"),(9500094,"是男人就下一百层"),(9500085,"付费后等待三秒，三只粪金龟会现身。你有六秒时间，每击败一只可得一件宝物。每人最多玩三次。")];
    static readonly (int,string)[] TrialEntries=[
        (9500118,"癫火封印上方的深坑跳跳乐，抵达底部领取传说武器宝箱。\n单人死亡直接结算本局。联机确认后提交全队投票。\n确定参加？"),
        (9500119,"从史东薇尔城正面跑到葛瑞克雾门前，获得两个随机护符增益。\n单人死亡直接结算本局。联机确认后提交全队投票。\n确定参加？"),
        (9500120,"竞技场生存挑战：5 秒后战车出现并追赶玩家，坚持 60 秒领取传说宝箱。\n单人死亡直接结算本局。联机确认后提交全队投票。\n确定参加？"),
        (9500121,"领取 800 卢恩并跳过本次挑战。\n联机确认后提交投票，最终选择由全队投票决定。\n确定选择？")];
    // 问号房的赌运事件（骰子赌徒、盲盒商人、词条重铸台）：原版对话菜单的选项、确认说明、结果与提示；编号与 native/src/native_talk.rs 的 GAMBLE_* 对应
    public static readonly (int,string)[] GambleEntries=[
        (9500130,"押 1000 卢恩"),(9500131,"押 3000 卢恩"),
        (9500132,"普通盲盒（1000 卢恩）"),(9500133,"高级盲盒（2500 卢恩）"),
        (9500134,"重铸右手武器（1000 卢恩）"),(9500135,"精炼右手武器（3000 卢恩）"),
        (9500136,"赌徒把两枚骨骰在掌心摇得作响。\n押 1000 卢恩：赢了拿回 2000，输了全归赌徒。\n确定下注？"),
        (9500137,"赌徒把两枚骨骰在掌心摇得作响。\n押 3000 卢恩：赢了拿回 6000，输了全归赌徒。\n确定下注？"),
        (9500138,"支付 1000 卢恩，随机获得一件本职业可用的武器或防具。\n稀有 65%　史诗 30%　传说 5%\n确定购买？"),
        (9500139,"支付 2500 卢恩，随机获得一件本职业可用的武器或防具。\n稀有 15%　史诗 55%　传说 30%\n确定购买？"),
        (9500140,"支付 1000 卢恩，把右手的词条武器换成同一把武器的另一种词条组合，1/5 的概率品质 +1。\n强化等级与战灰保留。确定重铸？"),
        (9500141,"支付 3000 卢恩，把右手的词条武器换成另一种词条组合，品质必定 +1；传说品质的武器不能精炼。\n强化等级与战灰保留。确定精炼？"),
        (9500142,"骰子落定，你赢了 1000 卢恩。"),(9500143,"骰子落定，你赢了 3000 卢恩。"),
        (9500144,"骰子落定，你输掉了 1000 卢恩。"),(9500145,"骰子落定，你输掉了 3000 卢恩。"),
        (9500146,"商人掀开黑布，把箱子里的装备交给你。"),
        (9500147,"火焰退去，武器换了一种形状。"),(9500148,"火焰烧得更旺，武器带着新的光泽回到你手中。"),
        (9500149,"卢恩不足，没有消耗任何东西。"),
        (9500150,"传说品质的武器不能精炼，没有消耗任何东西。"),
        (9500151,"右手没有词条武器，没有消耗任何东西。"),
        (9500152,"这把武器没有其他词条组合，没有消耗任何东西。"),
        (9500153,"暂时无法进行，没有消耗任何东西。"),
        (9500154,"这里的交易已经完成，请从传送门离开。"),
        // 重练炉：只有一个选项，价格每次翻倍，实际价格显示在画面上方的房间提示里（native_talk 的 FURNACE_*）
        (9500170,"重练右手武器"),
        (9500171,"把右手的词条武器投入炉火，换成同品质的另一把武器，词条组合随机。\n强化等级与战灰保留。价格从 1000 卢恩起，每重练一次翻倍，本次价格见画面上方。\n确定重练？"),
        (9500172,"炉火吞下武器，又吐出另一把。"),
        // 战斗法师（营地铁匠，native_talk 的 BATTLEMAGE_*）
        (9500180,"战斗法师：附加 S 补正"),
        (9500181,"右手武器附加智力 S 补正"),
        (9500182,"右手武器附加信仰 S 补正"),
        (9500183,"完成。右手武器获得了新的 S 补正，这项能力值也计入物理伤害。"),
        (9500184,"无法附加：需要右手装备近战武器，且这把武器还没有 S 补正（一局最多 8 把）。")];
    // 营地调灵（小红帽）：菜单项与提示；编号与 native/src/native_talk.rs 的 TUNE_* 对应
    static readonly (int,string)[] TuneEntries=[(9500155,"调灵（强化骨灰）"),
        (9500156,"没有可以调灵的骨灰，或身上没有强化石。需要背包里有尚未到 +10 的骨灰，且没有持有它的目标等级。"),
        (9500157,"调灵完成。之后刷出的骨灰最高可到这个等级。")];
    static readonly (int,string)[] ChestEntries=[(9500053,"三个宝箱，三份命运。每箱两成火药、两成守卫、六成宝物。")];
    static readonly (int,string)[] EndEntries=[(9500108,"结束本局"),(9500109,"结束本局并返回大赐福？联机时，全队将结束本局。"),(9500110,"当前无法结束本局。联机时请由房主操作。"),(9500111,"出售武器")];
    public static void Add(BND4 menu,bool chestOnly=false) {
        var file=menu.Files.Single(f=>f.ID==33);var fmg=FMG.Read(file.Bytes);
        foreach(var (id,text) in chestOnly?ChestEntries:Entries.Concat(EndEntries).Concat(StormEntries).Concat(TrialEntries).Concat(GambleEntries).Concat(TuneEntries).Concat(PriceEntries).Concat(MausoleumEntries)){fmg.Entries.RemoveAll(e=>e.ID==id);fmg.Entries.Add(new FMG.Entry(id,text));}
        file.Bytes=fmg.Write();
    }
    public static void AddTalismans(BND4 menu,BND4 regulation,BND4 item) {
        PARAM Read(string name) {var p=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));return p;}
        var materials=Read("EquipMtrlSetParam").Rows.ToDictionary(r=>r.ID);
        var names=FMG.Read(item.Files.Single(f=>f.ID==13).Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
        var file=menu.Files.Single(f=>f.ID==33);var text=FMG.Read(file.Bytes);
        void Set(int id,string value){text.Entries.RemoveAll(e=>e.ID==id);text.Entries.Add(new FMG.Entry(id,value));}
        Set(9500112,"没有可强化的护符。需要持有低阶护符，且尚未持有其强化目标。");
        Set(9500113,"用掉一块强化石，将所选护符兑换为高阶版本？已装备的护符会在原栏位替换。");
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
        var entries=chestOnly?ChestEntries:Entries.Concat(EndEntries).Concat(StormEntries).Concat(TrialEntries).Concat(GambleEntries).Concat(TuneEntries).Concat(PriceEntries).Concat(MausoleumEntries).ToArray();
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
