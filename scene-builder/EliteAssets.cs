using SoulsFormats;
static class EliteAssets {
    public static readonly int[] Buffs=[1000,1010,1020,1030,1040,1060,1070,1080,1090,1150,2060,2130,2180,2200,3000,3040];
    public const int First=9700000;
    public static void Build(PARAM effects,PARAM accessories,Action<string,PARAM> save) {
        for(int i=0;i<Buffs.Length;i++) {
            var accessory=accessories.Rows.Single(r=>r.ID==Buffs[i]);
            var source=effects.Rows.Single(r=>r.ID==Convert.ToInt32(accessory["refId"].Value));
            var row=new PARAM.Row(source){ID=First+i,Name="Tower elite permanent talisman"};
            if(effects.Rows.Any(r=>r.ID==row.ID))throw new Exception("Elite effect ID conflict");
            foreach(var (key,value) in new[]{("effectEndurance",-1),("spCategory",0),("categoryPriority",0),("saveCategory",-1),("dontDeleteOnDead",0)})row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
            effects.Rows.Add(row);
        }
        save("SpEffectParam",effects);
    }
    public static void Verify(PARAM effects,PARAM accessories,PARAM goods) {
        var changed=new HashSet<string>{"effectEndurance","spCategory","categoryPriority","saveCategory","dontDeleteOnDead"};
        for(int i=0;i<Buffs.Length;i++) {
            var accessory=accessories.Rows.Single(r=>r.ID==Buffs[i]);
            foreach(var field in new[]{"residentSpEffectId1","residentSpEffectId2","residentSpEffectId3","residentSpEffectId4"})if(Convert.ToInt32(accessory[field].Value)>0)throw new Exception("Unsupported multi-effect elite talisman");
            var source=effects.Rows.Single(r=>r.ID==Convert.ToInt32(accessory["refId"].Value));
            var row=effects.Rows.Single(r=>r.ID==First+i);
            foreach(var cell in source.Cells.Where(c=>!changed.Contains(c.Def.InternalName)))if(!Equals(cell.Value,row[cell.Def.InternalName].Value))throw new Exception("Elite talisman effect differs from original");
            if(Convert.ToSingle(row["effectEndurance"].Value)!=-1||Convert.ToInt32(row["saveCategory"].Value)!=-1)throw new Exception("Elite effect lifetime invalid");
        }
        foreach(int id in new[]{10020,8185}.Concat(Enumerable.Range(1000,26)).Concat(Enumerable.Range(1050,26)))if(!goods.Rows.Any(r=>r.ID==id))throw new Exception("Missing native tear/flask item");
        Console.WriteLine("Verified 16 original talisman effects, sacred tear and all 12 flask upgrades");
    }
}
