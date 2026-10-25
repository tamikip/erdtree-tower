using SoulsFormats;

static class SpiritAssets {
    static void Set(PARAM.Row r,string key,int value)=>r[key].Value=Convert.ChangeType(value,r[key].Value.GetType());
    public static void Build(Func<string,PARAM> read,Action<string,PARAM> save) {
        var goods=read("EquipParamGoods");
        foreach(var r in goods.Rows.Where(r=>Convert.ToInt32(r["goodsType"].Value) is 7 or 8)) {
            Set(r,"consumeMP",0);Set(r,"consumeHP",-1);
            Set(r,"isConsume",1);Set(r,"isSuppleItem",0);Set(r,"isFullSuppleItem",0);Set(r,"autoReplenishType",0);
        }
        save("EquipParamGoods",goods);
        var stones=read("BuddyStoneParam");
        foreach(var id in new[]{45000100,45010100,45020100}) {
            var stone=stones.Rows.Single(r=>r.ID==id);
            Set(stone,"disableParam_NT",0);Set(stone,"warnRegionEntityId",0);
            Set(stone,"summonedEventFlagId",45020968);
        }
        save("BuddyStoneParam",stones);
    }
    public static void Verify(Func<string,PARAM> read) {
        var ashes=read("EquipParamGoods").Rows.Where(r=>Convert.ToInt32(r["goodsType"].Value) is 7 or 8).ToList();
        if(ashes.Count<100||ashes.Any(r=>Convert.ToInt32(r["consumeMP"].Value)!=0||Convert.ToInt32(r["consumeHP"].Value)!=-1))throw new Exception("Spirit summons still consume HP or FP");
        if(ashes.Any(r=>Convert.ToInt32(r["isConsume"].Value)!=1||Convert.ToInt32(r["isSuppleItem"].Value)!=0||Convert.ToInt32(r["isFullSuppleItem"].Value)!=0||Convert.ToInt32(r["autoReplenishType"].Value)!=0))throw new Exception("Spirit ashes must be consumable without automatic refill");
        foreach(var id in new[]{45000100,45010100,45020100}) {
            var stone=read("BuddyStoneParam").Rows.Single(r=>r.ID==id);
            if(Convert.ToUInt32(stone["summonedEventFlagId"].Value)!=45020968)throw new Exception("Spirit flag overlaps native arena event");
            if(Convert.ToInt32(stone["disableParam_NT"].Value)!=0||Convert.ToUInt32(stone["overwriteActivateRegionEntityId"].Value)!=id+2001)throw new Exception("Arena spirit summon region missing");
        }
        Console.WriteLine($"Verified {ashes.Count} spirit ash levels consume no HP or FP");
    }
}
