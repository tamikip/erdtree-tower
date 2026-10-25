using SoulsFormats;

static class RequirementsAssets {
    static readonly (string Table,string[] Fields)[] Requirements=[
        ("EquipParamWeapon",["properStrength","properAgility","properMagic","properFaith","properLuck"]),
        ("Magic",["requirementIntellect","requirementFaith","requirementLuck"])
    ];
    public static void Build(Func<string,PARAM> read,Action<string,PARAM> save) {
        foreach(var (table,fields) in Requirements) {
            var param=read(table);
            foreach(var row in param.Rows)foreach(var field in fields)
                row[field].Value=Convert.ChangeType(0,row[field].Value.GetType());
            save(table,param);
        }
    }
    public static void Verify(Func<string,PARAM> read) {
        foreach(var (table,fields) in Requirements)
            foreach(var row in read(table).Rows)foreach(var field in fields)
                if(Convert.ToInt32(row[field].Value)!=0)throw new Exception($"Ability requirement remains: {table}:{row.ID}:{field}");
        Console.WriteLine("Verified weapons, sorceries and incantations have no ability requirements");
    }
}
