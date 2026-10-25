using SoulsFormats;

static class SmithAssets {
    public const int Voucher=7500100,ShopFirst=9510000,MaterialBase=700000;
    static void Set(PARAM.Row row,string name,int value)=>row[name].Value=Convert.ChangeType(value,row[name].Value.GetType());
    static void Text(BND4 binder,int table,int id,string text) {
        var file=binder.Files.Single(f=>f.ID==table);var fmg=FMG.Read(file.Bytes);
        if(fmg.Entries.Any(e=>e.ID==id))throw new Exception("Smith text ID conflict");
        fmg.Entries.Add(new FMG.Entry(id,text));fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=fmg.Write();
    }
    public static void Build(Func<string,PARAM> read,Action<string,PARAM> save,BND4 item) {
        var goods=read("EquipParamGoods");var materials=read("EquipMtrlSetParam");
        var weapons=read("EquipParamWeapon");var shops=read("ShopLineupParam");
        var voucher=new PARAM.Row(goods.Rows.Single(r=>r.ID==8000)){ID=Voucher,Name="Camp single reinforcement allowance"};
        foreach(var name in new[]{"refId_default","sellValue"})Set(voucher,name,-1);
        foreach(var name in new[]{"goodsType","isDiscard","isDeposit","isDrop"})Set(voucher,name,0);
        Set(voucher,"maxNum",1);
        foreach(var cell in voucher.Cells.Where(c=>c.Def.InternalName.StartsWith("enable_")))cell.Value=Convert.ChangeType(0,cell.Value.GetType());
        if(goods.Rows.Any(r=>r.ID==Voucher))throw new Exception("Smith voucher conflict");goods.Rows.Add(voucher);
        Text(item,10,Voucher,"营地强化凭证");
        foreach(var table in new[]{20,24})Text(item,table,Voucher,"本次营地可强化一件武器或将一件护符升至同系列最高阶。\n低强化武器可追赶持有武器的最高等效等级。与补充圣杯瓶共用一次服务机会，离开营地失效。");
        for(int offset=0;offset<=byte.MaxValue;offset++) {
            var row=new PARAM.Row(MaterialBase+offset,"One camp reinforcement",materials.AppliedParamdef);
            Set(row,"materialId01",Voucher);Set(row,"itemNum01",1);Set(row,"materialCate01",4);
            if(materials.Rows.Any(r=>r.ID==row.ID))throw new Exception("Smith material conflict");materials.Rows.Add(row);
        }
        // Native reinforcement uses the origin chain and shop category, not isEnhance.
        foreach(var row in weapons.Rows.Where(r=>Convert.ToInt32(r["reinforceShopCategory"].Value) is >=1 and <=4&&Enumerable.Range(1,25).Any(level=>Convert.ToInt32(r[$"originEquipWep{level}"].Value)>=0))) {
            Set(row,"materialSetId",MaterialBase);Set(row,"reinforcePrice",0);
        }
        var accessories=read("EquipParamAccessory");
        var names=FMG.Read(item.Files.Single(f=>f.ID==13).Bytes).Entries
            .Where(e=>e.ID<5000&&!string.IsNullOrWhiteSpace(e.Text)&&accessories.Rows.Any(r=>r.ID==e.ID)).ToList();
        int index=0;
        foreach(var previous in names.OrderBy(e=>e.ID)) {
            var family=previous.Text.EndsWith("＋１")||previous.Text.EndsWith("＋２")?previous.Text[..^2]:previous.Text;
            var next=names.SingleOrDefault(e=>e.Text==family+"＋２")??names.SingleOrDefault(e=>e.Text==family+"＋１");
            if(next==null||next.ID==previous.ID)continue;
            if(index>=100)throw new Exception("Smith shop range overflow");
            var material=new PARAM.Row(MaterialBase+256+index,"Talisman tier exchange",materials.AppliedParamdef);
            Set(material,"materialId01",Voucher);Set(material,"itemNum01",1);Set(material,"materialCate01",4);
            Set(material,"materialId02",previous.ID);Set(material,"itemNum02",1);Set(material,"materialCate02",2);
            materials.Rows.Add(material);
            var shop=new PARAM.Row(ShopFirst+index,"Camp talisman upgrade",shops.AppliedParamdef);
            Set(shop,"equipId",next.ID);Set(shop,"equipType",2);Set(shop,"value",0);Set(shop,"costType",0);
            Set(shop,"mtrlId",material.ID);Set(shop,"sellQuantity",-1);Set(shop,"setNum",1);
            foreach(var field in new[]{"iconId","nameMsgId","menuTitleMsgId","menuIconId"})Set(shop,field,-1);
            shop["value_Magnification"].Value=1f;
            if(shops.Rows.Any(r=>r.ID==shop.ID))throw new Exception("Smith shop conflict");shops.Rows.Add(shop);index++;
        }
        if(index<6)throw new Exception("Native talisman tier names were not resolved");
        foreach(var shop in shops.Rows.Where(r=>r.ID>=ShopFirst&&r.ID<ShopFirst+index)) {
            var target=names.Single(e=>e.ID==Convert.ToInt32(shop["equipId"].Value));
            if(target.Text.EndsWith("＋１")&&names.Any(e=>e.Text==target.Text[..^2]+"＋２"))throw new Exception("Talisman exchange must target the maximum tier");
            var material=materials.Rows.Single(r=>r.ID==Convert.ToInt32(shop["mtrlId"].Value));
            if(Convert.ToInt32(material["materialId02"].Value)==target.ID)throw new Exception("Talisman exchange consumes its own output");
        }
        save("EquipParamGoods",goods);save("EquipMtrlSetParam",materials);save("EquipParamWeapon",weapons);save("ShopLineupParam",shops);
        Console.WriteLine($"Built native smith: one voucher per upgrade, {index} original talisman tier exchanges");
    }
    public static void Verify(Func<string,PARAM> read) {
        var goods=read("EquipParamGoods").Rows.Single(r=>r.ID==Voucher);
        if(goods.ID>=0x800000)throw new Exception("Goods ID overlaps the indexed gaitem handle bit");
        if(Convert.ToInt32(goods["maxNum"].Value)!=1||Convert.ToInt32(goods["isDiscard"].Value)!=0||Convert.ToInt32(goods["isDeposit"].Value)!=0||Convert.ToInt32(goods["isDrop"].Value)!=0)throw new Exception("Voucher can escape camp accounting");
        var materials=read("EquipMtrlSetParam");var weapons=read("EquipParamWeapon");var reinforce=read("ReinforceParamWeapon");
        foreach(var weapon in weapons.Rows.Where(r=>Convert.ToInt32(r["reinforceShopCategory"].Value) is >=1 and <=4&&Enumerable.Range(1,25).Any(level=>Convert.ToInt32(r[$"originEquipWep{level}"].Value)>=0))) {
            if(Convert.ToInt32(weapon["materialSetId"].Value)!=MaterialBase)throw new Exception($"Upgradeable weapon {weapon.ID} still uses vanilla materials");
            if(Convert.ToInt32(weapon["reinforcePrice"].Value)!=0)throw new Exception("Camp weapon upgrade costs runes");
            foreach(var offset in reinforce.Rows.Select(r=>Convert.ToInt32(r["materialSetId"].Value)).Distinct()) {
                var material=materials.Rows.Single(r=>r.ID==Convert.ToInt32(weapon["materialSetId"].Value)+offset);
                if(Convert.ToInt32(material["materialId01"].Value)!=Voucher||Convert.ToInt32(material["itemNum01"].Value)!=1)throw new Exception("Weapon upgrade bypasses single allowance");
            }
        }
        var shops=read("ShopLineupParam").Rows.Where(r=>r.ID>=ShopFirst&&r.ID<ShopFirst+100).ToList();
        if(shops.Count<6)throw new Exception("Talisman upgrade choices missing");
        foreach(var shop in shops) {
            var material=materials.Rows.Single(r=>r.ID==Convert.ToInt32(shop["mtrlId"].Value));
            if(Convert.ToInt32(shop["equipType"].Value)!=2||Convert.ToInt32(shop["value"].Value)!=0||Convert.ToInt32(material["materialId01"].Value)!=Voucher||Convert.ToInt32(material["itemNum01"].Value)!=1||Convert.ToInt32(material["materialCate02"].Value)!=2||Convert.ToInt32(material["itemNum02"].Value)!=1)throw new Exception("Invalid native talisman exchange");
        }
    }
}
