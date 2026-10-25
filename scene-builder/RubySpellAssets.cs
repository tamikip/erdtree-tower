using SoulsFormats;
using System.Text.Json;

static class RubySpellAssets {
    const string Assets="outputs/erdtree-tower/scene-assets";
    static readonly string[] Tables=["Magic","Bullet","AtkParam_Pc","SpEffectParam","SpEffectVfxParam","BehaviorParam_PC","EquipParamGoods"];
    static int I(PARAM.Row r,string k)=>Convert.ToInt32(r[k].Value);
    static void Set(PARAM.Row r,string k,int v)=>r[k].Value=Convert.ChangeType(v,r[k].Value.GetType());
    static string Def(string name)=>name switch {"Magic"=>"MagicParam","Bullet"=>"BulletParam","AtkParam_Pc"=>"AtkParam","SpEffectParam"=>"SpEffect","SpEffectVfxParam"=>"SpEffectVfx","BehaviorParam_PC"=>"BehaviorParam",_=>name};
    static PARAM Read(BND4 reg,string name) {
        var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
        var d=PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{Def(name)}.xml");
        if(p.ParamType!=d.ParamType||p.DetectedSize!=d.GetRowSize())throw new Exception($"Ruby spell layout mismatch: {name}");
        p.ApplyParamdef(d);return p;
    }
    static bool Equal(PARAM.Row a,PARAM.Row b)=>a.Cells.Zip(b.Cells).All(c=>System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(c.First.Value,c.Second.Value));
    static IEnumerable<(string Field,string Table,int Id)> Refs(string table,PARAM.Row row) {
        var result=new List<(string,string,int)>();
        void Add(string field,string target) {int id=I(row,field);if(id>0)result.Add((field,target,id));}
        string Target(int category)=>category switch {0=>"AtkParam_Pc",1=>"Bullet",2=>"SpEffectParam",_=>throw new Exception("Unknown Ruby spell reference category")};
        switch(table) {
            case "Magic":
                for(int j=1;j<=10;j++)if(I(row,$"refId{j}")>=0)result.Add(($"refId{j}",Target(I(row,$"refCategory{j}")),I(row,$"refId{j}")));
                foreach(var k in new[]{"replaceMagicId","ReplacementMagic1","ReplacementMagic2","ReplacementMagic3","ReplacementMagic4"})Add(k,"Magic");
                Add("limitCancelSpEffectId","SpEffectParam");break;
            case "Bullet":
                Add("atkId_Bullet","AtkParam_Pc");Add("HitBulletID","Bullet");Add("intervalCreateBulletId","Bullet");
                foreach(var k in new[]{"spEffectIDForShooter","spEffectId0","spEffectId1","spEffectId2","spEffectId3","spEffectId4"})Add(k,"SpEffectParam");break;
            case "AtkParam_Pc":
                for(int j=0;j<5;j++)Add($"spEffectId{j}","SpEffectParam");break;
            case "SpEffectParam":
                foreach(var k in new[]{"replaceSpEffectId","cycleOccurrenceSpEffectId","atkOccurrenceSpEffectId","accumuOverFireId","accumuUnderFireId","applyIdOnGetSoul","spiritDeathSpEffectId"})Add(k,"SpEffectParam");
                foreach(var k in new[]{"vfxId","vfxId1","vfxId2","vfxId3","vfxId4","vfxId5","vfxId6","vfxId7"})Add(k,"SpEffectVfxParam");
                Add("behaviorId","BehaviorParam_PC");break;
            case "BehaviorParam_PC":Add("refId",Target(I(row,"refType")));break;
        }
        return result;
    }
    public static void Export(string game,string source,string output) {
        Directory.CreateDirectory(output);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(source,"regulation.bin"));
        var vanilla=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
        var rows=Tables.ToDictionary(t=>t,t=>Read(reg,t).Rows.ToDictionary(r=>r.ID));
        var baseline=Tables.ToDictionary(t=>t,t=>Read(vanilla,t).Rows.ToDictionary(r=>r.ID));
        var text=new Dictionary<int,Dictionary<int,string>>();
        foreach(var file in Directory.GetFiles(Path.Combine(source,"msg/engus"),"*.dcx").Order())foreach(var f in BND4.Read(file).Files) {
            if(!text.ContainsKey(f.ID))text[f.ID]=new();
            foreach(var e in FMG.Read(f.Bytes).Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text)))text[f.ID][e.ID]=e.Text;
        }
        var roots=rows["EquipParamGoods"].Keys.Where(i=>!baseline["EquipParamGoods"].ContainsKey(i)&&rows["Magic"].ContainsKey(i)&&text[10].ContainsKey(i)&&text[24].ContainsKey(i)).Order().ToArray();
        if(roots.Length!=101)throw new Exception($"Expected 101 Ruby spells, got {roots.Length}");
        var reachable=new HashSet<(string Table,int Id)>();var missing=new HashSet<string>();
        void Visit(string table,int id) {
            if(!reachable.Add((table,id)))return;
            foreach(var (field,target,refId) in Refs(table,rows[table][id])) {
                if(rows[target].ContainsKey(refId))Visit(target,refId);
                else if(target=="SpEffectVfxParam")missing.Add($"{table}\t{id}\t{field}\t{target}\t{refId}");
                else throw new Exception($"Missing executable Ruby dependency: {table}:{id}:{field}:{refId}");
            }
        }
        foreach(int id in roots){Visit("Magic",id);reachable.Add(("EquipParamGoods",id));}
        var added=reachable.Where(k=>!baseline[k.Table].ContainsKey(k.Id)).ToHashSet();
        foreach(var k in reachable.Except(added)) {
            if(!Equal(rows[k.Table][k.Id],baseline[k.Table][k.Id]))throw new Exception($"Ruby needs an isolated shared-row override: {k}");
            if(Refs(k.Table,rows[k.Table][k.Id]).Any(r=>added.Contains((r.Table,r.Id))))throw new Exception($"Ruby needs an isolated ancestor: {k}");
        }
        var translations=File.ReadAllLines(Path.Combine(Assets,"ruby-spell-translations.tsv")).Select(l=>l.Split('\t')).ToDictionary(p=>int.Parse(p[0]));
        var catalog=new List<string>();var details=new List<string>();var rarities=new List<string>();var original=new List<object>();
        string Escape(string s)=>s.Replace("\r","").Replace("\n","\\n").Replace("\t"," ");
        foreach(int id in roots) {
            var magic=rows["Magic"][id];var goods=rows["EquipParamGoods"][id];
            if(I(magic,"ezStateBehaviorType")!=0)throw new Exception("Unexpected Ruby spell catalyst category");
            Set(goods,"goodsType",5);
            foreach(var k in new[]{"requirementIntellect","requirementFaith","requirementLuck"})Set(magic,k,0);
            var tr=translations[id];string name=tr[1],info=tr[2];uint tagged=0x40000000u|(uint)id;
            string stats=$"属性要求：无\\n施法工具：法杖\\n专注消耗：{I(magic,"mp")}\\n记忆空格：{I(magic,"slotLength")}";
            catalog.Add($"sorcery|{id}|{name}");
            details.Add(string.Join('\t',new[]{tagged.ToString(),I(goods,"iconId").ToString(),name,Escape(info),Escape(info),stats,""}));
            rarities.Add($"{tagged}\t{I(goods,"rarity")}");
            original.Add(new{id,name=text[10][id],caption=text[24][id]});
        }
        reg.Files.RemoveAll(f=>!Tables.Contains(Path.GetFileNameWithoutExtension(f.Name)));
        foreach(var f in reg.Files) {
            string table=Path.GetFileNameWithoutExtension(f.Name);var p=Read(reg,table);
            p.Rows.Clear();p.Rows.AddRange(added.Where(k=>k.Table==table).OrderBy(k=>k.Id).Select(k=>rows[table][k.Id]));f.Bytes=p.Write();
        }
        reg.Write(Path.Combine(output,"ruby-spells.bnd.dcx"));
        File.WriteAllLines(Path.Combine(output,"ruby-spells-catalog.txt"),catalog);
        File.WriteAllLines(Path.Combine(output,"ruby-spells-details.tsv"),details);
        File.WriteAllLines(Path.Combine(output,"ruby-spells-rarities.tsv"),rarities);
        File.WriteAllLines(Path.Combine(output,"ruby-spells-missing-vfx.tsv"),missing.Order());
        File.WriteAllText(Path.Combine(output,"ruby-spells-original.json"),JsonSerializer.Serialize(original,new JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine($"Exported {roots.Length} Ruby spells, {added.Count} new parameter rows; {missing.Count} pre-existing missing visual references");
    }
    public static void Build(Func<string,PARAM> read,Action<string,PARAM> save,BND4 messages) {
        var payload=BND4.Read(Path.Combine(Assets,"ruby-spells.bnd.dcx"));
        foreach(var file in payload.Files) {
            string name=Path.GetFileNameWithoutExtension(file.Name);var source=Read(payload,name);var target=read(name);var occupied=target.Rows.ToDictionary(r=>r.ID);
            foreach(var row in source.Rows) {
                if(occupied.TryGetValue(row.ID,out var old)) {if(!Equal(old,row))throw new Exception($"Ruby spell ID collision: {name}:{row.ID}");}
                else target.Rows.Add(new PARAM.Row(row));
            }
            save(name,target);
        }
        var lines=File.ReadAllLines(Path.Combine(Assets,"ruby-spells-details.tsv")).Select(l=>l.Split('\t')).ToArray();
        foreach(int table in new[]{10,20,24}) {
            var file=messages.Files.Single(f=>f.ID==table);var fmg=FMG.Read(file.Bytes);
            foreach(var line in lines) {
                int id=(int)(uint.Parse(line[0])&0x0fffffff);string text=line[table==10?2:table==20?3:4].Replace("\\n","\n");
                var old=fmg.Entries.SingleOrDefault(e=>e.ID==id);
                if(old!=null&&old.Text!=text)throw new Exception($"Ruby spell text collision: {table}:{id}");
                if(old==null)fmg.Entries.Add(new FMG.Entry(id,text));
            }
            fmg.Entries.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=fmg.Write();
        }
        var missing=File.ReadAllLines(Path.Combine(Assets,"ruby-spells-missing-vfx.tsv")).ToHashSet();
        var targetIds=Tables.Where(t=>t!="EquipParamGoods").ToDictionary(t=>t,t=>read(t).Rows.Select(r=>r.ID).ToHashSet());
        foreach(var f in payload.Files) {
            var table=Path.GetFileNameWithoutExtension(f.Name);
            foreach(var r in Read(payload,table).Rows)foreach(var (field,target,id) in Refs(table,r))
                if(!targetIds[target].Contains(id)&&!missing.Contains($"{table}\t{r.ID}\t{field}\t{target}\t{id}"))throw new Exception($"Dangling Ruby spell dependency: {table}:{r.ID}:{field}:{id}");
        }
    }
    public static void Patch(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate spell staging directory required");
        Directory.CreateDirectory(output);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var before=reg.Files.ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());var cache=new Dictionary<string,PARAM>();
        PARAM Get(string name) {if(!cache.ContainsKey(name))cache[name]=Read(reg,name);return cache[name];}
        void Save(string name,PARAM p) {p.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes=p.Write();}
        var msgPath="msg/zhocn/item_dlc02.msgbnd.dcx";var messages=BND4.Read(Path.Combine(input,msgPath));
        var oldText=messages.Files.ToDictionary(f=>f.ID,f=>FMG.Read(f.Bytes).Entries.Select(e=>(e.ID,e.Text)).ToArray());
        Build(Get,Save,messages);
        var path=Path.Combine(output,"regulation.bin");SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        if(new FileInfo(path).Length>4799*1024)throw new Exception("Spell regulation exceeds engine size limit");
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files) {
            if(!f.Bytes.SequenceEqual(reg.Files.Single(r=>r.Name==f.Name).Bytes))throw new Exception("Spell regulation roundtrip mismatch");
            if(f.Bytes.SequenceEqual(before[f.Name]))continue;
            var table=Path.GetFileNameWithoutExtension(f.Name);var old=PARAM.Read(before[f.Name]);old.ApplyParamdef(Get(table).AppliedParamdef);
            var actual=Read(check,table).Rows.ToDictionary(r=>r.ID);
            foreach(var r in old.Rows)if(!actual.ContainsKey(r.ID)||!Equal(r,actual[r.ID]))throw new Exception($"Original parameter changed: {table}:{r.ID}");
        }
        Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,msgPath))!);messages.Write(Path.Combine(output,msgPath));
        foreach(var f in BND4.Read(Path.Combine(output,msgPath)).Files) {
            var entries=FMG.Read(f.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
            foreach(var e in oldText[f.ID])if(!entries.TryGetValue(e.ID,out var text)||text!=e.Text)throw new Exception("Original item text changed");
        }
        string Hash(string p)=>Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(p)));
        File.WriteAllText(Path.Combine(output,"ruby-spells-build.json"),JsonSerializer.Serialize(new{spells=101,originalParametersPreserved=true,originalTextPreserved=true,executableReferencesVerified=true,sourceMissingVfx=File.ReadAllLines(Path.Combine(Assets,"ruby-spells-missing-vfx.tsv")).Length,sourceRegulationSha256=Hash(Path.Combine(input,"regulation.bin")),regulationSha256=Hash(path),textSha256=Hash(Path.Combine(output,msgPath)),bytes=new FileInfo(path).Length},new JsonSerializerOptions{WriteIndented=true}));
        Console.WriteLine("Verified 101 Ruby spells, dependency references, original parameters/text, encryption roundtrip and size limit");
    }
}
