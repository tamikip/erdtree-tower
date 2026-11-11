using SoulsFormats;
using System.Text.Json;

static class LocalizationAssets {
    static readonly JsonSerializerOptions Json=new(){WriteIndented=true,Encoder=System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping};
    static readonly string[] Packs=["item_dlc02.msgbnd.dcx","menu_dlc02.msgbnd.dcx"];
    static void Extract(string game,string output) {
        var paths=new[]{"zhocn","engus"}.SelectMany(lang=>Packs.Select(pack=>$"/msg/{lang}/{pack}")).ToHashSet();
        foreach(var archive in new[]{"Data0","Data1","Data2","Data3","DLC"}) {
            if(paths.Count==0)break;
            if(!File.Exists(Path.Combine(game,archive+".bhd")))continue;
            using var header=UXM.CryptographyUtility.DecryptRsa(Path.Combine(game,archive+".bhd"),UXM.ArchiveKeys.EldenRingKeys[archive]);
            var files=BHD5.Read(header,BHD5.Game.EldenRing).Buckets.SelectMany(b=>b).ToDictionary(f=>f.FileNameHash);
            using var data=File.OpenRead(Path.Combine(game,archive+".bdt"));
            foreach(var path in paths.ToArray()) {
                ulong hash=0;foreach(char ch in path)hash=unchecked(hash*0x85+ch);
                if(!files.TryGetValue(hash,out var file))continue;
                var target=Path.Combine(output,path.TrimStart('/'));Directory.CreateDirectory(Path.GetDirectoryName(target)!);
                File.WriteAllBytes(target,file.ReadFile(data));paths.Remove(path);
            }
        }
        if(paths.Count!=0)throw new Exception("Missing original language resources: "+string.Join(',',paths));
    }
    public static void Export(string game,string input,string output) {
        Directory.CreateDirectory(output);var originals=Path.Combine(output,"originals");Extract(game,originals);
        var pairs=new Dictionary<string,HashSet<string>>();var names=new Dictionary<string,HashSet<string>>();var changes=new List<object>();var contextual=new List<object>();
        foreach(var pack in Packs) {
            var zh=BND4.Read(Path.Combine(originals,"msg/zhocn",pack));var en=BND4.Read(Path.Combine(originals,"msg/engus",pack));
            var mod=BND4.Read(Path.Combine(input,"msg/zhocn",pack));
            foreach(var file in zh.Files) {
                var english=en.Files.SingleOrDefault(f=>f.ID==file.ID);if(english==null)continue;
                var byId=FMG.Read(english.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
                foreach(var entry in FMG.Read(file.Bytes).Entries) {
                    if(string.IsNullOrWhiteSpace(entry.Text)||!byId.TryGetValue(entry.ID,out var translated)||string.IsNullOrWhiteSpace(translated))continue;
                    contextual.Add(new{pack,table=file.ID,id=entry.ID,source=entry.Text,text=translated});
                    if(!pairs.TryGetValue(entry.Text,out var values))pairs[entry.Text]=values=new();values.Add(translated);
                    if(file.Name.Contains("Name")) {if(!names.TryGetValue(entry.Text,out var labels))names[entry.Text]=labels=new();labels.Add(translated);}
                }
            }
            if(pack.StartsWith("item_")) {
                var mergedZh=ItemTextTables.Read(zh);var mergedEn=ItemTextTables.Read(en);
                foreach(var (table,name) in new[]{(25,"WeaponCaption"),(26,"ProtectorCaption")}) {
                    var translated=mergedEn[name].Entries.ToDictionary(e=>e.ID,e=>e.Text);
                    foreach(var entry in mergedZh[name].Entries)
                        if(translated.TryGetValue(entry.ID,out var english))contextual.Add(new{pack,table,id=entry.ID,source=entry.Text,text=english});
                }
            }
            foreach(var file in mod.Files) {
                var vanilla=zh.Files.SingleOrDefault(f=>f.ID==file.ID);
                var before=vanilla==null?new Dictionary<int,string>():FMG.Read(vanilla.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
                foreach(var entry in FMG.Read(file.Bytes).Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text)))
                    if(!before.TryGetValue(entry.ID,out var previous)||previous!=entry.Text)
                        changes.Add(new{pack,table=file.ID,name=Path.GetFileName(file.Name),id=entry.ID,text=entry.Text});
            }
        }
        File.WriteAllText(Path.Combine(output,"official-en.json"),JsonSerializer.Serialize(pairs.Where(p=>p.Value.Count==1).ToDictionary(p=>p.Key,p=>p.Value.Single()),Json));
        File.WriteAllText(Path.Combine(output,"official-en-variants.json"),JsonSerializer.Serialize(pairs.Where(p=>p.Value.Count>1).ToDictionary(p=>p.Key,p=>p.Value.OrderBy(v=>v,StringComparer.Ordinal).ToArray()),Json));
        File.WriteAllText(Path.Combine(output,"official-context.json"),JsonSerializer.Serialize(contextual,Json));
        File.WriteAllText(Path.Combine(output,"official-names-en.json"),JsonSerializer.Serialize(names.Where(p=>p.Value.Count==1).ToDictionary(p=>p.Key,p=>p.Value.Single()),Json));
        File.WriteAllText(Path.Combine(output,"custom-fmg.json"),JsonSerializer.Serialize(changes,Json));
        Console.WriteLine($"Exported {changes.Count} modified FMG entries and {pairs.Count(p=>p.Value.Count==1)} unambiguous official translations");
    }
    public record Translation(string pack,int table,int id,string source,string text);
    public static void BuildTheme(string input,string output,string textures) {
        foreach(var quality in new[]{"hi","low"}) {
            var relative=$"menu/{quality}/01_common.tpf.dcx";
            var tpf=TPF.Read(Path.Combine(input,relative));var before=tpf.Textures.ToDictionary(t=>t.Name,t=>t.Bytes);
            var changed=new HashSet<string>();
            foreach(var name in new[]{"title","loading1","loading2","loading3","tips1","tips2"}) {
                string key="SB_TowerTheme_"+name;int index=tpf.Textures.FindIndex(t=>t.Name==key);
                if(index<0)throw new Exception("Missing theme texture: "+key);
                var bytes=File.ReadAllBytes(Path.Combine(textures,name+".dds"));var dds=new DDS(bytes);
                if(dds.dwWidth!=1920||dds.dwHeight!=1080||bytes.Length!=128+1920*1080*4)throw new Exception("Invalid English theme DDS: "+name);
                tpf.Textures[index]=new TPF.Texture(key,9,0,bytes,TPF.TPFPlatform.PC){Mipmaps=1};changed.Add(key);
            }
            var target=Path.Combine(output,relative);Directory.CreateDirectory(Path.GetDirectoryName(target)!);tpf.Write(target);
            var actual=TPF.Read(target).Textures.ToDictionary(t=>t.Name,t=>t.Bytes);
            if(actual.Count!=before.Count||before.Any(p=>!changed.Contains(p.Key)&&!actual[p.Key].SequenceEqual(p.Value)))throw new Exception("Unrelated UI textures changed");
            foreach(var name in changed)if(!actual[name].SequenceEqual(tpf.Textures.Single(t=>t.Name==name).Bytes))throw new Exception("English theme roundtrip failed");
            Console.WriteLine($"Verified {quality} English theme: six replacements, all other textures retained");
        }
    }
    public static void Build(string game,string input,string output,string translations) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate English staging output required");
        var entries=JsonSerializer.Deserialize<Translation[]>(File.ReadAllText(translations))??throw new Exception("Missing English translations");
        var translated=entries.ToDictionary(e=>(e.pack,e.table,e.id));
        foreach(var entry in entries)if(string.IsNullOrWhiteSpace(entry.text)||entry.text.Any(c=>c>='\u3400'&&c<='\u9fff'))throw new Exception($"Untranslated English FMG entry: {entry.pack}/{entry.table}/{entry.id}");
        var originals=Path.Combine(output,"originals");Extract(game,originals);int changed=0,removed=0;
        foreach(var pack in Packs) {
            var zh=BND4.Read(Path.Combine(originals,"msg/zhocn",pack));var en=BND4.Read(Path.Combine(originals,"msg/engus",pack));
            var mod=BND4.Read(Path.Combine(input,"msg/zhocn",pack));var expected=new Dictionary<int,Dictionary<int,string>>();
            foreach(var file in mod.Files) {
                var original=zh.Files.SingleOrDefault(f=>f.ID==file.ID);var english=en.Files.SingleOrDefault(f=>f.ID==file.ID);
                var baseline=original==null?new Dictionary<int,string>():FMG.Read(original.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);
                var current=FMG.Read(file.Bytes);
                if(english==null) {
                    if(current.Entries.Count==baseline.Count&&current.Entries.All(e=>baseline.TryGetValue(e.ID,out var old)&&old==e.Text))continue;
                    throw new Exception($"Modified table has no English counterpart: {pack}/{file.ID}");
                }
                var target=FMG.Read(english.Bytes);var currentIds=current.Entries.Select(e=>e.ID).ToHashSet();
                removed+=target.Entries.RemoveAll(e=>baseline.ContainsKey(e.ID)&&!currentIds.Contains(e.ID));
                foreach(var entry in current.Entries) {
                    if(baseline.TryGetValue(entry.ID,out var old)&&old==entry.Text)continue;
                    string text=entry.Text;
                    if(!string.IsNullOrWhiteSpace(text)) {
                        if(!translated.TryGetValue((pack,file.ID,entry.ID),out var value)||value.source!=text)throw new Exception($"Missing or stale English translation: {pack}/{file.ID}/{entry.ID}");
                        text=value.text;changed++;
                    }
                    target.Entries.RemoveAll(e=>e.ID==entry.ID);target.Entries.Add(new FMG.Entry(entry.ID,text));
                }
                english.Bytes=target.Write();expected[file.ID]=target.Entries.ToDictionary(e=>e.ID,e=>e.Text);
            }
            var path=Path.Combine(output,"msg/engus",pack);Directory.CreateDirectory(Path.GetDirectoryName(path)!);en.Write(path);
            foreach(var file in BND4.Read(path).Files.Where(f=>expected.ContainsKey(f.ID))) {
                var actual=FMG.Read(file.Bytes).Entries.ToDictionary(e=>e.ID,e=>e.Text);var wanted=expected[file.ID];
                if(actual.Count!=wanted.Count||wanted.Any(p=>!actual.TryGetValue(p.Key,out var text)||text!=p.Value))throw new Exception("English text roundtrip failed");
            }
        }
        if(changed!=entries.Length)throw new Exception("English translation count mismatch");
        File.WriteAllText(Path.Combine(output,"english-text-build.json"),JsonSerializer.Serialize(new{changed,removed,source=Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(translations)))},Json));
        Console.WriteLine($"Verified {changed} English mod text entries and {removed} removals; unchanged base-game English text retained");
    }
}
