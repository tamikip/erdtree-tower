using SoulsFormats;
using System.Text;

static class NativeAssets {
    record EnemySpec(int Act,string Tier,int Model,int Runes);
    static EnemySpec[] EnemyPool(string root)=>File.ReadAllLines(Path.Combine(root,"enemy-pools.txt")).Select(line=>{
        var p=line.Split('|');return new EnemySpec(int.Parse(p[0]),p[1],int.Parse(p[2]),int.Parse(p[3]));
    }).ToArray();
    public const int ShopBase=9500000,ShopCount=37,GoodsBase=9500000,ChestLot=45010900;
    public static int StockFlag(int index)=>index<21?45020800+index*8:index<26?45020700+(index-21)*8:45020600+(index-26)*8;
    static byte[] Num(int n)=>n>=-64&&n<=63?[(byte)(n+64)]:new byte[]{0x82}.Concat(BitConverter.GetBytes(n)).ToArray();
    static byte[] Expr(byte[] b)=>b.Concat(new byte[]{0xa1}).ToArray();
    static byte[] Fn(int id,params byte[][] args)=>Num(id).Concat(args.SelectMany(a=>a)).Append((byte)(0x84+args.Length)).ToArray();
    static byte[] Eq(byte[] a,int b)=>a.Concat(Num(b)).Append((byte)0x95).ToArray();
    static byte[] And(byte[] a,byte[] b)=>a.Concat(b).Append((byte)0x98).ToArray();
    static ESD.CommandCall Cmd(int id,params int[] args)=>new(1,id,args.Select(n=>Expr(Num(n))).ToArray());
    static ESD.Condition Go(int state,byte[] expr)=>new(state,Expr(expr));
    static byte[] Flag(int id)=>Fn(15,Num(id));
    static ESD Talk(int service) {
        var esd=new ESD(true,3);var states=new Dictionary<long,ESD.State>();esd.StateGroups[1]=states;
        for(int i=0;i<=9;i++)states[i]=new ESD.State();
        int basis=service==2?45020000:45000000;
        states[0].EntryCommands.Add(Cmd(35));
        states[0].Conditions.Add(Go(1,And(Flag(basis+993),Fn(56,Num(service==1?9500001:6000)))));
        states[1].Conditions.Add(Go(8,service==0?Flag(basis+995):Num(0)));
        states[1].Conditions.Add(Go(2,Num(1)));
        states[2].EntryCommands.AddRange(new[]{Cmd(110),Cmd(20),Cmd(19,1,9500000+service,-1),Cmd(19,2,9500003,-1)});
        states[2].Conditions.Add(Go(3,Num(1)));
        states[3].EntryCommands.Add(Cmd(10,1));
        states[3].Conditions.Add(Go(4,Eq(Fn(59,Num(1),Num(0)),0)));
        states[4].Conditions.Add(Go(5,Eq(Fn(23),1)));states[4].Conditions.Add(Go(0,Num(1)));
        if(service==2) {
            states[5].EntryCommands.Add(Cmd(22,ShopBase,ShopBase+ShopCount-1));states[5].Conditions.Add(Go(6,Num(1)));
            states[6].Conditions.Add(Go(0,Eq(Fn(59,Num(5),Num(0)),0)));
        } else {
            states[5].EntryCommands.AddRange(new[]{Cmd(11,basis+998,0),Cmd(11,basis+999,0),Cmd(11,basis+996+service,1)});
            states[5].Conditions.Add(Go(6,Flag(basis+998)));
            states[6].Conditions.Add(Go(9,Flag(basis+999)));states[6].Conditions.Add(Go(7,Num(1)));
        }
        foreach(var (state,msg) in new[]{(7,9500010),(8,9500011),(9,9500012)}) {
            states[state].EntryCommands.Add(Cmd(17,7,msg,1,0,1));
            states[state].Conditions.Add(Go(0,Eq(Fn(58,Num(0)),0)));
        }
        if(service==1) {
            states[2].EntryCommands.Clear();states[2].EntryCommands.AddRange(new[]{Cmd(110),Cmd(20)});
            foreach(var (choice,text) in new[]{(1,9500001),(2,9500041),(99,9500003)})states[2].EntryCommands.Add(Cmd(19,choice,text,-1));
            states[4].Conditions.Clear();
            foreach(int choice in new[]{1,2})states[4].Conditions.Add(Go(8,And(Eq(Fn(23),choice),Flag(basis+995))));
            foreach(var (choice,state,command,menu) in new[]{(1,5,Cmd(24,0),9),(2,10,Cmd(22,SmithAssets.ShopFirst,SmithAssets.ShopFirst+99),5)}) {
                states[4].Conditions.Add(Go(state,Eq(Fn(23),choice)));
                states[state]=new ESD.State();states[state].EntryCommands.Add(command);
                states[state].Conditions.Add(Go(0,And(Eq(Fn(59,Num(menu),Num(0)),0),Eq(Fn(58,Num(0)),0))));
            }
            states[4].Conditions.Add(Go(0,Num(1)));
        }
        if(service==0) {
            states[2].EntryCommands.Clear();states[2].EntryCommands.AddRange(new[]{Cmd(110),Cmd(20),Cmd(19,1,9500000,-1),Cmd(19,2,9500044,-1),Cmd(19,99,9500003,-1)});
            states[4].Conditions.Clear();
            states[4].Conditions.Add(Go(5,Eq(Fn(23),1)));states[4].Conditions.Add(Go(10,Eq(Fn(23),2)));states[4].Conditions.Add(Go(0,Num(1)));
            states[5].EntryCommands.Insert(0,Cmd(11,basis+997,0));
            states[10]=new ESD.State();states[10].EntryCommands.AddRange(new[]{Cmd(11,basis+998,0),Cmd(11,basis+999,0),Cmd(11,basis+997,1),Cmd(11,basis+996,1)});
            states[10].Conditions.Add(Go(6,Flag(basis+998)));
        }
        return esd;
    }
    public static void Build(string game,string output,string cache) {
        Directory.CreateDirectory(output);
        var enemySource=Path.GetFullPath("outputs/erdtree-tower/scene-assets/enemy-pools.txt");
        var enemyOutput=Path.GetFullPath(Path.Combine(output,"enemy-pools.txt"));
        if(enemySource!=enemyOutput)File.Copy(enemySource,enemyOutput,true);
        var rewardSource=Path.GetFullPath("outputs/erdtree-tower/scene-assets/reward-catalog.txt");
        var rewardOutput=Path.GetFullPath(Path.Combine(output,"reward-catalog.txt"));
        if(rewardSource!=rewardOutput)File.Copy(rewardSource,rewardOutput,true);
        var extrasSource=Path.GetFullPath("outputs/erdtree-tower/scene-assets/shop-extras.txt");
        var extrasOutput=Path.GetFullPath(Path.Combine(output,"shop-extras.txt"));
        if(extrasSource!=extrasOutput)File.Copy(extrasSource,extrasOutput,true);
        var regulation=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        var defs=Path.GetFullPath("work/SoulsRandomizers/diste/Defs");
        PARAM Read(string name,string def=null) {
            var p=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize(Path.Combine(defs,(def??name)+".xml")));return p;
        }
        void Save(string name,PARAM p) {p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes=p.Write();}
        var enemies=Read("NpcParam");
        var think=Read("NpcThinkParam");
        foreach(var spec in EnemyPool(output)) {
            int model=spec.Model;
            if(!think.Rows.Any(r=>r.ID==model*10000))throw new Exception($"Missing enemy AI: {model}");
            int id=900000000+model*10000;
            if(enemies.Rows.Any(r=>r.ID==id))throw new Exception("Tower NPC parameter conflict");
            var row=new PARAM.Row(enemies.Rows.Single(r=>r.ID==model*10000)){ID=id,Name="Tower rune-only enemy"};
            Set(row,"itemLotId_enemy",-1);Set(row,"itemLotId_map",-1);Set(row,"isSoulGetByBoss",0);
            Set(row,"getSoul",spec.Runes);
            enemies.Rows.Add(row);
        }
        if(!think.Rows.Any(r=>r.ID==41912000))throw new Exception("Scarab AI missing");
        var scarab=new PARAM.Row(enemies.Rows.Single(r=>r.ID==41912000)){ID=941912000,Name="Tower timed scarab"};
        Set(scarab,"itemLotId_enemy",-1);Set(scarab,"itemLotId_map",-1);Set(scarab,"getSoul",0);
        enemies.Rows.Add(scarab);
        Save("NpcParam",enemies);
        var goods=Read("EquipParamGoods");var shops=Read("ShopLineupParam");var lots=Read("ItemLotParam_map","ItemLotParam");
        var messages=BND4.Read(Path.Combine(cache,"item_dlc02.msgbnd.dcx"));
        BossAssets.Build(output,name=>Read(name),(name,param)=>Save(name,param),messages);
        enemies=Read("NpcParam");BalanceAssets.Apply(enemies,output);Save("NpcParam",enemies);
        void AddText(BND4 b,int id,IEnumerable<(int,string)> entries) {
            var f=b.Files.Single(f=>f.ID==id);var t=FMG.Read(f.Bytes);
            foreach(var (key,value) in entries) {if(t.Entries.Any(e=>e.ID==key))throw new Exception("Text ID conflict");t.Entries.Add(new FMG.Entry(key,value));}
            t.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));f.Bytes=t.Write();
        }
        string[] blessingNames={"铁壁反击","蓄势重击","闪避锋芒","鲜血脉动","魔力连携","余烬之火","生命赐福","耐力赐福"};
        string[] blessingEffects={"降低物理伤害并增强防御反击；升级提高减伤。","增强蓄力攻击；二级增加跳跃攻击，三级增加突刺反击。","加快耐力恢复；二级增加装备重量上限，三级进一步提高。","附近出血时提升攻击；二级追加连续攻击增益，三级强化连击增益。","增强魔法；二级提高增益，三级缩短施法时间。","增强火伤并增加物理承伤；另获火焰减伤，升级提高火焰减伤。","提高最大生命；升级提高增益，不直接补血。","提高最大耐力；升级提高增益。"};
        for(int i=0;i<8;i++) {
            if(goods.Rows.Any(r=>r.ID==GoodsBase+i))throw new Exception("Goods ID conflict");
            var row=new PARAM.Row(goods.Rows.Single(r=>r.ID==8000)){ID=GoodsBase+i,Name="Tower blessing"};
            Set(row,"goodsType",0);Set(row,"refId_default",-1);Set(row,"maxNum",1);Set(row,"sellValue",0);
            foreach(var cell in row.Cells.Where(c=>c.Def.InternalName.StartsWith("enable_")))cell.Value=Convert.ChangeType(0,cell.Value.GetType());
            goods.Rows.Add(row);
        }
        AddText(messages,10,blessingNames.Select((n,i)=>(GoodsBase+i,"赐福："+n)));
        foreach(int table in new[]{20,24})AddText(messages,table,blessingNames.Select((n,i)=>(GoodsBase+i,$"{blessingEffects[i]}\n购买后自动获得或升一级，最高三级，不占护符槽，不与同名护符效果重复叠加；重新开始旅程后失效。")));
        for(int i=0;i<ShopCount;i++) {
            if(shops.Rows.Any(r=>r.ID==ShopBase+i))throw new Exception("Shop ID conflict");
            var row=new PARAM.Row(ShopBase+i,"Tower randomized stock",shops.AppliedParamdef);
            Set(row,"equipId",2000000);Set(row,"equipType",0);Set(row,"value",1500);Set(row,"costType",0);
            foreach(var field in new[]{"iconId","nameMsgId","menuTitleMsgId","menuIconId","mtrlId"})Set(row,field,-1);
            Set(row,"setNum",1);row["value_Magnification"].Value=1f;
            Set(row,"sellQuantity",1);Set(row,"eventFlag_forStock",StockFlag(i));
            if(i==22){Set(row,"equipId",8185);Set(row,"equipType",3);Set(row,"value",5000);Set(row,"sellQuantity",-1);}
            shops.Rows.Add(row);
        }
        var lot=new PARAM.Row(ChestLot,"Tower native chest",lots.AppliedParamdef);
        Set(lot,"lotItemId01",2000000);Set(lot,"lotItemCategory01",2);Set(lot,"lotItemBasePoint01",1000);Set(lot,"lotItemNum01",1);Set(lot,"getItemFlagId",45010995);
        if(lots.Rows.Any(r=>r.ID==ChestLot))throw new Exception("Chest lot ID conflict");lots.Rows.Add(lot);
        var actions=Read("ActionButtonParam");
        var portal=new PARAM.Row(actions.Rows.Single(r=>r.ID==9200)){ID=9500000,Name="Tower route portal"};
        Set(portal,"textId",9500004);Set(portal,"angle",180);Set(portal,"allowAngle",180);portal["radius"].Value=3f;
        if(actions.Rows.Any(r=>r.ID==portal.ID))throw new Exception("Action button ID conflict");actions.Rows.Add(portal);
        var smithAction=new PARAM.Row(actions.Rows.Single(r=>r.ID==6000)){ID=9500001,Name="Tower large smith talk"};
        smithAction["radius"].Value=6f;Set(smithAction,"angle",180);Set(smithAction,"allowAngle",180);actions.Rows.Add(smithAction);
        var chestAction=new PARAM.Row(actions.Rows.Single(r=>r.ID==8000)){ID=9500002,Name="Tower chest front interaction"};
        Set(chestAction,"textId",9500005);Set(chestAction,"invalidFlag",0);actions.Rows.Add(chestAction);
        var lockedAction=new PARAM.Row(chestAction){ID=9500003,Name="Tower locked chest"};
        Set(lockedAction,"textId",9500072);actions.Rows.Add(lockedAction);
        Save("EquipParamGoods",goods);Save("ShopLineupParam",shops);Save("ItemLotParam_map",lots);Save("ActionButtonParam",actions);
        var saleWeapons=Read("EquipParamWeapon");WeaponSaleAssets.Apply(saleWeapons);Save("EquipParamWeapon",saleWeapons);
        SmithAssets.Build(name=>Read(name),Save,messages);
        RequirementsAssets.Build(name=>Read(name,name=="Magic"?"MagicParam":name),Save);
        SpiritAssets.Build(name=>Read(name),Save);
        ContractAssets.Build(Read("SpEffectParam","SpEffect"),Save);
        PassiveAssets.Build(Read("SpEffectParam","SpEffect"),Save);
        EliteAssets.Build(Read("SpEffectParam","SpEffect"),Read("EquipParamAccessory"),Save);
        AffixAssets.Build(name=>Read(name,name=="SpEffectParam"?"SpEffect":name),Save,messages,"outputs/erdtree-tower/scene-assets",output);
        AdvancedAffixAssets.Build(name=>Read(name,name=="SpEffectParam"?"SpEffect":name),Save,messages,"outputs/erdtree-tower/scene-assets",output);
        RubySpellAssets.Build(name=>Read(name,name switch {"Magic"=>"MagicParam","Bullet"=>"BulletParam","AtkParam_Pc"=>"AtkParam","SpEffectParam"=>"SpEffect","SpEffectVfxParam"=>"SpEffectVfx","BehaviorParam_PC"=>"BehaviorParam",_=>name}),Save,messages);
        Directory.CreateDirectory(output);SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),regulation);
        var shopSource=Path.GetFullPath("outputs/erdtree-tower/scene-assets/shop-catalog.txt");
        var shopOutput=Path.GetFullPath(Path.Combine(output,"shop-catalog.txt"));
        if(shopSource!=shopOutput)File.Copy(shopSource,shopOutput,true);
        var menu=BND4.Read(Path.Combine(cache,"menu_dlc02.msgbnd.dcx"));
        PartyStatusText.Add(menu);
        RecoveryText.Add(menu);
        RecoveryText.AddTalismans(menu,regulation,messages);
        AddText(menu,33,new[]{(9500080,"捕捉粪金龟 · 1000 卢恩"),(9500081,"询问规则"),(9500082,"稍等片刻。"),(9500083,"三次机会已用完，祝你一路顺风。"),(9500084,"需要 1000 卢恩。"),(9500085,"付费后等待三秒，三只粪金龟会现身。你有六秒时间，每击败一只可得一件宝物。每人最多玩三次。")});
        AddText(menu,32,new[]{(9500004,"进入爬塔路线")});
        AddText(menu,32,new[]{(9500005,"打开宝箱")});
        AddText(menu,32,new[]{(9500072,"用石剑钥匙打开")});
        AddText(menu,33,new[]{(9500072,"需要石剑钥匙 ×1"),(9500073,"消耗一把石剑钥匙，可获得武器或滴露。")});
        AddText(menu,33,new[]{(9500000,"补给：2 红 1 蓝"),(9500044,"补给：1 红 2 蓝"),(9500001,"强化武器"),(9500002,"查看行商货物"),(9500003,"离开")});
        AddText(menu,33,new[]{(9500020,"开始新局"),(9500021,"继续旅程"),(9500022,"选择流浪骑士"),(9500023,"选择观星者"),(9500024,"选择预言家"),(9500025,"选择武士")});
        AddText(menu,33,new[]{(9500010,"服务已完成。此处篝火只能选择一项服务。"),(9500011,"此处篝火的服务已经用完。"),(9500012,"未能完成服务。请检查瓶数、武器及背包后重试。")});
        AddText(menu,33,new[]{(9500030,"没有可继续的旅程，请选择开始新局。")});
        AddText(menu,33,new[]{(9500060,"选择剑士"),(9500061,"选择勇者"),(9500062,"选择盗贼"),(9500063,"选择囚犯"),(9500064,"选择密使"),(9500065,"选择一贫如洗")});
        AddText(menu,33,new[]{(9500056,"本次交互未完成。请先击败守卫；若领取物品失败，请清理背包后重试。守卫加载异常时可退出重进，事件进度会保留。")});
        AddText(menu,33,new[]{(9500052,"献金求宝"),(9500055,"祭坛已经沉寂。赠礼已尽，请从传送门继续旅程。"),(9500057,"卢恩不足，需要 800 卢恩。")});
        AddText(menu,33,new[]{(9500050,"献血求宝"),(9500051,"挑战守卫"),(9500053,"三个宝箱，三份命运。每箱三成火药、三成守卫、四成宝物。"),(9500054,"贪欲已惊醒守卫。先击败他们，再来交谈。")});
        AddText(menu,33,new[]{(9500040,"升级")});
        AddText(menu,33,new[]{(9500045,"重新分配能力值")});
        AddText(menu,33,new[]{(9500046,"提升圣杯瓶恢复量"),(9500047,"消耗 1 枚圣杯滴露，提升红、蓝圣杯瓶恢复量一级？不会补充剩余次数。"),(9500048,"需要 1 枚圣杯滴露。"),(9500049,"红、蓝圣杯瓶恢复量已提升。")});
        AddText(menu,33,new[]{(9500070,"暂时无法强化：圣杯瓶已达到 +12 上限，或瓶子尚未就绪。")});
        AddText(menu,33,new[]{(9500071,"调配灵药")});
        var campText=FMG.Read(menu.Files.Single(f=>f.ID==33).Bytes);
        campText.Entries.Single(e=>e.ID==9500000).Text="补给：2 红 1 蓝＋灵药";
        campText.Entries.Single(e=>e.ID==9500044).Text="补给：1 红 2 蓝＋灵药";
        menu.Files.Single(f=>f.ID==33).Bytes=campText.Write();
        AddText(menu,33,new[]{(9500041,"强化护符"),(9500042,"更换战灰"),(9500043,"记忆魔法／祷告")});
        var msgDir=Path.Combine(output,"msg","zhocn");Directory.CreateDirectory(msgDir);menu.Write(Path.Combine(msgDir,"menu_dlc02.msgbnd.dcx"));messages.Write(Path.Combine(msgDir,"item_dlc02.msgbnd.dcx"));
        var global=BND4.Read(Path.Combine(cache,"m00_00_00_00.talkesdbnd.dcx"));
        for(int service=0;service<3;service++) {
            var esd=Talk(service);var bytes=esd.Write();var verified=ESD.Read(bytes);
            if(verified.StateGroups[1].Count!=(service!=2?11:10) || verified.StateGroups[1].Values.SelectMany(s=>s.Conditions).Any(c=>c.TargetState.HasValue&&!verified.StateGroups[1].ContainsKey(c.TargetState.Value)))throw new Exception("ESD roundtrip failed");
            global.Files.Add(new BinderFile(Binder.FileFlags.Flag1,global.Files.Max(f=>f.ID)+1,$"N:\\GR\\data\\INTERROOT_win64\\script\\talk\\m00_00_00_00\\t95000000{service}.esd",bytes));
        }
        var talkDir=Path.Combine(output,"script","talk");Directory.CreateDirectory(talkDir);global.Write(Path.Combine(talkDir,"m00_00_00_00.talkesdbnd.dcx"));
        foreach(var map in new[]{"m45_00_00_00","m45_01_00_00","m45_02_00_00"}) {
            var local=BND4.Read(global.Write());local.Files=local.Files.Where(f=>f.Name.Contains("t95000000")).ToList();
            foreach(var f in local.Files)f.Name=f.Name.Replace("m00_00_00_00",map);
            local.Write(Path.Combine(talkDir,map+".talkesdbnd.dcx"));
        }
        var verifiedReg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        var verifyLot=PARAM.Read(verifiedReg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ItemLotParam_map").Bytes);
        verifyLot.ApplyParamdef(lots.AppliedParamdef);
        if((int)verifyLot.Rows.Single(r=>r.ID==ChestLot)["lotItemCategory01"].Value!=2)throw new Exception("Item lot roundtrip mismatch");
        var verifyShop=PARAM.Read(verifiedReg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam").Bytes);
        if(verifyShop.Rows.Count(r=>r.ID>=ShopBase&&r.ID<ShopBase+ShopCount)!=ShopCount)throw new Exception("Shop roundtrip mismatch");
        Console.WriteLine($"Built native dialogs, rune shop, {File.ReadAllLines(shopOutput).Length} stock candidates, blessing goods and chest lot");
    }
    public static void Verify(string output) {
        RegionalAssets.Verify(output);
        SceneValidation.Verify(output);
        var regulation=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        var verifiedParams=new Dictionary<string,PARAM>();
        PARAM Read(string name) {
            if(verifiedParams.TryGetValue(name,out var cached))return cached;
            var p=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
            p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{(name=="Magic"?"MagicParam":name=="SpEffectParam"?"SpEffect":name)}.xml"));
            verifiedParams[name]=p;return p;
        }
        SmithAssets.Verify(Read);
        PassiveAssets.Verify(Read("SpEffectParam"),Read("EquipParamGoods"));
        var epicIds=File.ReadAllLines("outputs/erdtree-tower/scene-assets/epic-weapons.txt").Select(int.Parse).ToArray();
        var catalogIds=File.ReadAllLines("outputs/erdtree-tower/scene-assets/reward-catalog.txt").Select(l=>l.Split('|')).Where(p=>p[0]=="weapon").Select(p=>int.Parse(p[1])).ToHashSet();
        var expectedEpic=Read("EquipParamWeapon").Rows.Where(r=>catalogIds.Contains(r.ID)&&Convert.ToInt32(r["rarity"].Value)==2).Select(r=>r.ID).ToHashSet();
        if(epicIds.Length!=epicIds.Distinct().Count()||!expectedEpic.SetEquals(epicIds))throw new Exception("Epic weapon catalog differs from original rarity");
        Console.WriteLine($"Verified {epicIds.Length} original epic weapons in elite pool");
        BossAssets.Verify(output,name=>Read(name));
        RequirementsAssets.Verify(Read);
        SpiritAssets.Verify(Read);
        ContractAssets.Verify(Read("SpEffectParam"),Read("EquipParamWeapon"),Read("EquipParamGoods"),Read("ReinforceParamWeapon"));
        EliteAssets.Verify(Read("SpEffectParam"),Read("EquipParamAccessory"),Read("EquipParamGoods"));
        var chestAction=Read("ActionButtonParam").Rows.Single(r=>r.ID==9500002);
        var lockedAction=Read("ActionButtonParam").Rows.Single(r=>r.ID==9500003);
        if(Convert.ToInt32(lockedAction["textId"].Value)!=9500072 || Convert.ToInt32(Read("EquipParamGoods").Rows.Single(r=>r.ID==8000)["maxNum"].Value)<1)throw new Exception("Locked chest key or action missing");
        foreach(var cell in chestAction.Cells.Where(c=>c.Def.InternalName!="textId"))if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,lockedAction[cell.Def.InternalName].Value))throw new Exception("Locked chest interaction geometry differs from normal chest");
        var physick=Read("EquipParamGoods");
        foreach(var (id,replacement,consumed) in new[]{(250,251,1),(251,250,0)}) {
            var bottle=physick.Rows.Single(r=>r.ID==id);
            if(Convert.ToInt32(bottle["replaceItemId"].Value)!=replacement||Convert.ToInt32(bottle["isConsume"].Value)!=consumed||Convert.ToInt32(bottle["maxNum"].Value)!=1)throw new Exception("Native Physick bottle lifecycle mismatch");
        }
        if(physick.Rows.Count(r=>r.ID>=11000&&r.ID<=11031)!=32)throw new Exception("Base-game crystal tears missing");
        var nativeChestAction=Read("ActionButtonParam").Rows.Single(r=>r.ID==8000);
        foreach(var cell in nativeChestAction.Cells.Where(c=>!new[]{"textId","invalidFlag"}.Contains(c.Def.InternalName)))
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,chestAction[cell.Def.InternalName].Value))throw new Exception("Chest interaction must use original chest geometry");
        if(Convert.ToInt32(chestAction["textId"].Value)!=9500005||Convert.ToInt32(chestAction["invalidFlag"].Value)!=0)throw new Exception("Invalid chest action text or gate");
        var enemies=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="NpcParam").Bytes);
        enemies.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml"));
        var scarab=enemies.Rows.Single(r=>r.ID==941912000);
        foreach(var (field,value) in new[]{("itemLotId_enemy",-1),("itemLotId_map",-1),("getSoul",0),("hp",(int)BalanceAssets.ScaleHp(154))})
            if(Convert.ToInt32(scarab[field].Value)!=value)throw new Exception($"Timed scarab parameter mismatch: {field}");
        var pool=EnemyPool(output);
        if(pool.Select(e=>e.Model).Distinct().Count()!=pool.Length)throw new Exception("Enemy pools must use distinct models");
        foreach(int act in new[]{1,2,3}) {
            if(pool.Count(e=>e.Act==act&&e.Tier=="weak")<8||pool.Count(e=>e.Act==act&&e.Tier=="elite")<6||pool.Count(e=>e.Act==act&&e.Tier=="boss")<(act==1?4:1))throw new Exception("Incomplete act enemy pool");
        }
        foreach(var spec in pool) {
            int model=spec.Model;
            var row=enemies.Rows.Single(r=>r.ID==900000000+model*10000);
            var original=enemies.Rows.Single(r=>r.ID==model*10000);
            if(Convert.ToInt64(row["hp"].Value)!=BalanceAssets.ScaleHp(Convert.ToInt64(original["hp"].Value)))throw new Exception("Enemy HP balance mismatch");
            foreach(var cell in row.Cells.Where(c=>!new[]{"hp","itemLotId_enemy","itemLotId_map","getSoul","isSoulGetByBoss"}.Contains(c.Def.InternalName)))
                if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,original[cell.Def.InternalName].Value))throw new Exception($"Enemy must retain native parameters: {model}/{cell.Def.InternalName}");
            if(Convert.ToInt32(row["itemLotId_enemy"].Value)!=-1||Convert.ToInt32(row["itemLotId_map"].Value)!=-1||Convert.ToInt32(row["getSoul"].Value)!=BalanceAssets.Scale(spec.Runes)||Convert.ToInt32(row["isSoulGetByBoss"].Value)!=0)throw new Exception($"Invalid rune-only enemy: {model}");
        }
        var shops=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam").Bytes);
        shops.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/ShopLineupParam.xml"));
        foreach(var row in shops.Rows.Where(r=>r.ID>=ShopBase&&r.ID<ShopBase+ShopCount)) {
            foreach(var field in new[]{"iconId","nameMsgId","menuTitleMsgId","menuIconId","mtrlId"})
                if(Convert.ToInt32(row[field].Value)!=-1)throw new Exception($"Shop {row.ID}: {field} must use native item data (-1)");
            if(Convert.ToInt32(row["setNum"].Value)!=1 || Convert.ToSingle(row["value_Magnification"].Value)!=1f)throw new Exception("Invalid native shop defaults");
        }
        var item=BND4.Read(Path.Combine(output,"msg","zhocn","item_dlc02.msgbnd.dcx"));
        foreach(var line in File.ReadAllLines(Path.Combine(output,"reward-catalog.txt"))) {
            var parts=line.Split('|');var id=int.Parse(parts[1]);
            var (param,fmg)=parts[0] switch {"weapon"=>("EquipParamWeapon",11),"armor"=>("EquipParamProtector",12),"talisman"=>("EquipParamAccessory",13),"sorcery" or "incantation" or "spirit"=>("EquipParamGoods",10),_=>throw new Exception("Invalid reward category")};
            var rows=Read(param);
            if(!rows.Rows.Any(r=>r.ID==id)||!FMG.Read(item.Files.Single(f=>f.ID==fmg).Bytes).Entries.Any(e=>e.ID==id&&e.Text==parts[2]))throw new Exception($"Reward item/name mismatch: {line}");
        }
        foreach(var line in File.ReadAllLines(Path.Combine(output,"shop-extras.txt"))) {
            var p=line.Split('|');var param=p[1]=="0"?"EquipParamWeapon":"EquipParamGoods";var fmg=p[1]=="0"?11:10;
            var rows=Read(param);
            if(!rows.Rows.Any(r=>r.ID==int.Parse(p[2]))||!FMG.Read(item.Files.Single(f=>f.ID==fmg).Bytes).Entries.Any(e=>e.ID==int.Parse(p[2])&&e.Text==p[5]))throw new Exception("Shop extra missing: "+line);
        }
        foreach(var line in File.ReadAllLines(Path.Combine(output,"shop-catalog.txt"))) {
            var p=line.Split('|');var param=p[0]=="0"?"EquipParamWeapon":"EquipParamGem";var fmg=p[0]=="0"?11:35;
            if(!Read(param).Rows.Any(r=>r.ID==int.Parse(p[1]))||!FMG.Read(item.Files.Single(f=>f.ID==fmg).Bytes).Entries.Any(e=>e.ID==int.Parse(p[1])&&e.Text==p[3]))throw new Exception("Shop catalog item missing: "+line);
        }
        var bgm=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="WwiseValueToStrParam_BgmBossChrIdConv").Bytes);
        if(!bgm.Rows.Any(r=>r.ID==203000))throw new Exception("Battle BGM missing");
        for(int area=0;area<3;area++) {
            uint basis=45000000u+(uint)area*10000;
            string mapName=$"m45_0{area}_00_00";
            var events=EMEVD.Read(Path.Combine(output,"event",mapName+".emevd.dcx"));
            var music=events.Events.Single(e=>e.ID==basis+985);
            if(music.Instructions.Count(i=>i.Bank==2010&&i.ID==10)!=3)throw new Exception("Battle BGM lifecycle missing");
            var portal=events.Events.Single(e=>e.ID==basis+983);
            if(!portal.Instructions.Any(i=>i.Bank==3&&i.ID==24&&BitConverter.ToUInt32(i.ArgData,8)==basis+1900))throw new Exception("Battle portal target mismatch");
            var map=MSBE.Read(Path.Combine(output,"map","mapstudio",mapName+".msb.dcx"));
            if(map.Parts.Enemies.Single(e=>e.EntityID==basis+100).TalkID!=2000)throw new Exception("Battle summon talk missing");
            if(!map.Regions.GetEntries().Any(r=>r.EntityID==basis+2101))throw new Exception("Battle summon region missing");
        }
        if(shops.Rows.Count(r=>r.ID>=ShopBase&&r.ID<ShopBase+ShopCount)!=ShopCount)throw new Exception("Expanded shop rows missing");
        var tear=shops.Rows.Single(r=>r.ID==ShopBase+22);
        foreach(var (field,value) in new[]{("equipId",8185),("equipType",3),("value",5000),("sellQuantity",-1)})
            if(Convert.ToInt32(tear[field].Value)!=value)throw new Exception("Invalid larval tear shop stock");
        if(!Read("EquipParamGoods").Rows.Any(r=>r.ID==8185)||!FMG.Read(item.Files.Single(f=>f.ID==10).Bytes).Entries.Any(e=>e.ID==8185&&!string.IsNullOrWhiteSpace(e.Text)))throw new Exception("Original larval tear missing");
        var accessories=PARAM.Read(regulation.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamAccessory").Bytes);
        var accessoryText=FMG.Read(item.Files.Single(f=>f.ID==13).Bytes);
        foreach(var id in new[]{4000,4001,4002,2130,2180,2060,1150,1030,1032,2160,2080,2081,3000,3001,3070,2020,4021,4022,1000,1001,1002,1020,1021,1022}) {
            if(!accessories.Rows.Any(r=>r.ID==id)||!accessoryText.Entries.Any(e=>e.ID==id&&!string.IsNullOrWhiteSpace(e.Text)))throw new Exception($"Original talisman missing: {id}");
        }
        foreach(var id in new[]{10,20,24}) {
            var fmg=FMG.Read(item.Files.Single(f=>f.ID==id).Bytes);
            if(fmg.Entries.Count(e=>e.ID>=GoodsBase&&e.ID<GoodsBase+8&& !string.IsNullOrWhiteSpace(e.Text))!=8)throw new Exception("Blessing text missing");
        }
        var menu=BND4.Read(Path.Combine(output,"msg","zhocn","menu_dlc02.msgbnd.dcx"));
        var scarabText=FMG.Read(menu.Files.Single(f=>f.ID==33).Bytes);
        foreach(int id in Enumerable.Range(9500080,6))if(!scarabText.Entries.Any(e=>e.ID==id&&!string.IsNullOrWhiteSpace(e.Text)))throw new Exception("Scarab dialogue missing");
        if(!menu.Files.Any(f=>Path.GetFileNameWithoutExtension(f.Name)=="EventTextForTalk_dlc02"))throw new Exception("Current-version text binder required; renaming legacy text is not sufficient");
        FMG TextTable(string name)=>FMG.Read(menu.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
        if(!TextTable("ActionButtonText").Entries.Any(e=>e.ID==9500004&&e.Text=="进入爬塔路线"))throw new Exception("Portal action text missing");
        if(!TextTable("ActionButtonText").Entries.Any(e=>e.ID==9500005&&e.Text=="打开宝箱"))throw new Exception("Chest action text missing");
        var talkText=TextTable("EventTextForTalk");
        foreach(var id in new[]{9500045,9500071,22001000,22001001,22001002})
            if(!talkText.Entries.Any(e=>e.ID==id&&!string.IsNullOrWhiteSpace(e.Text)))throw new Exception($"Rebirth dialogue missing: {id}");
        foreach(var id in Enumerable.Range(9500060,6))if(talkText.Entries.Count(e=>e.ID==id&&!string.IsNullOrWhiteSpace(e.Text))!=1)throw new Exception("Starting class dialogue text missing");
        var classes=Read("CharaInitParam");
        foreach(var id in Enumerable.Range(3000,10)) {
            var row=classes.Rows.Single(r=>r.ID==id);
            if(Convert.ToInt32(row["baseVit"].Value)<1||Convert.ToInt32(row["soulLv"].Value)<1)throw new Exception("Invalid starting class attributes");
            foreach(var (field,param) in new[]{("equip_Wep_Right","EquipParamWeapon"),("equip_Wep_Left","EquipParamWeapon"),("equip_Subwep_Right","EquipParamWeapon"),("equip_Subwep_Left","EquipParamWeapon"),("equip_Helm","EquipParamProtector"),("equip_Armer","EquipParamProtector"),("equip_Gaunt","EquipParamProtector"),("equip_Leg","EquipParamProtector"),("equip_Spell_01","EquipParamGoods"),("equip_Spell_02","EquipParamGoods")}) {
                var equipmentId=Convert.ToInt32(row[field].Value);
                if(equipmentId>=0&&!Read(param).Rows.Any(r=>r.ID==equipmentId))throw new Exception($"Missing starting equipment {id}:{field}:{equipmentId}");
            }
        }
        Console.WriteLine("Verified all ten original class attribute, equipment and starting spell rows");
        foreach(var id in Enumerable.Range(9500050,8))if(talkText.Entries.Count(e=>e.ID==id&&!string.IsNullOrWhiteSpace(e.Text))!=1)throw new Exception("Regional dialogue text missing");
        foreach(var id in new[]{9500000,9500001,9500002,9500003,9500010,9500011,9500012,9500020,9500021,9500022,9500023,9500024,9500025,9500030,9500040,9500041,9500042,9500043,9500044}) {
            if(talkText.Entries.Count(e=>e.ID==id&&!string.IsNullOrWhiteSpace(e.Text))!=1)throw new Exception($"EventTextForTalk missing or duplicate: {id}");
        }
        var hub=EMEVD.Read(Path.Combine(output,"event","m11_10_00_00.emevd.dcx"));
        if(!hub.Events.Single(e=>e.ID==11100980).Instructions.Any(i=>i.Bank==2003&&i.ID==72&&BitConverter.ToSingle(i.ArgData,0)==1f))throw new Exception("Hub bloodstain reset missing");
        foreach(var map in new[]{"m00_00_00_00","m45_00_00_00","m45_01_00_00","m45_02_00_00"}) {
            var binder=BND4.Read(Path.Combine(output,"script","talk",map+".talkesdbnd.dcx"));
            var scripts=binder.Files.Where(f=>f.Name.Contains("t95000000")).ToList();
            if(scripts.Count!=3)throw new Exception("Native NPC scripts missing");
            foreach(var script in scripts) {
                var esd=ESD.Read(script.Bytes);var states=esd.StateGroups[1];
                if(states.Count!=(script.Name.Contains("t950000002")?10:11)||states.Values.SelectMany(s=>s.Conditions).Any(c=>c.TargetState.HasValue&&!states.ContainsKey(c.TargetState.Value)))throw new Exception("Invalid native dialog state target");
                if(script.Name.Contains("t950000001")&&new[]{24,22}.Any(id=>!states.Values.SelectMany(s=>s.EntryCommands).Any(c=>c.CommandBank==1&&c.CommandID==id)))throw new Exception("Native smith menu missing");
                if(script.Name.Contains("t950000002")&&!states.Values.SelectMany(s=>s.EntryCommands).Any(c=>c.CommandBank==1&&c.CommandID==22))throw new Exception("Merchant does not open native shop");
            }
        }
        Console.WriteLine("Verified localized native menu text, 24 blessing text entries, 12 ESD scripts and native shop commands");
    }
    static void Set(PARAM.Row row,string field,int value)=>row[field].Value=Convert.ChangeType(value,row[field].Value.GetType());
}
