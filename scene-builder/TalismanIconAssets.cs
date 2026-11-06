using System.Text;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using SoulsFormats;

// 词条护符的重绘图标：每类护符一张（四个稀有度共用），按 talisman-definitions.tsv 里各类第一次出现的顺序编号 65100 起。
// 图标由 scene-builder/talisman_art.py 从 talisman-art/ 的图集切成 reward-icons/{编号}.rgba，并改写 talisman-details.tsv 的图标列。
// talisman-icon-patch：只改 regulation 里这些护符的 iconId（Linux 上也能运行），其余参数逐字节不变，可重复运行。
// talisman-icon-menu：在菜单贴图 01_common（hi、low）追加图集 SB_Icon_TowerTalismans，需要 Oodle（Windows）。
static class TalismanIconAssets {
    public const int IconBase=65100;
    const string Atlas="SB_Icon_TowerTalismans";
    const int Columns=12;
    /// 护符编号 → 图标编号；缺任何一张重绘图标时返回 null（继续用原包图标）
    public static Dictionary<int,int>? Icons(string input) {
        var defs=File.ReadAllLines(Path.Combine(input,"talisman-definitions.tsv")).Select(l=>l.Split('\t')).ToArray();
        var kinds=defs.Select(p=>p[4]).Distinct().ToList();
        if(!Enumerable.Range(0,kinds.Count).All(i=>File.Exists(Path.Combine(input,"reward-icons",$"{IconBase+i}.rgba"))))return null;
        return defs.ToDictionary(p=>int.Parse(p[0]),p=>IconBase+kinds.IndexOf(p[4]));
    }
    static Dictionary<int,int> Required(string input)=>Icons(input)??throw new Exception("Talisman artwork missing: run scene-builder/talisman_art.py first");
    public static void PatchParams(string input,string output) {
        if(Path.GetFullPath(input)==Path.GetFullPath(output))throw new Exception("Separate staging output required");
        var icons=Required(input);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name.Replace('\\','/'))=="EquipParamAccessory");
        var def=PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/EquipParamAccessory.xml");
        var table=PARAM.Read(file.Bytes);table.ApplyParamdef(def);
        var original=PARAM.Read(file.Bytes);original.ApplyParamdef(def);
        int changed=0;
        foreach(var (id,icon) in icons) {
            var row=table.Rows.SingleOrDefault(r=>r.ID==id)??throw new Exception($"Missing talisman {id}");
            if(row.Name?.StartsWith("Tower talisman ")!=true)throw new Exception($"Not a tower talisman: {id}");
            if(Convert.ToInt32(row["iconId"].Value)==icon)continue;
            row["iconId"].Value=Convert.ChangeType(icon,row["iconId"].Value.GetType());changed++;
        }
        if(changed>0)file.Bytes=table.Write();
        var others=reg.Files.Where(f=>f!=file).ToDictionary(f=>f.Name,f=>f.Bytes.ToArray());
        Directory.CreateDirectory(output);var path=Path.Combine(output,"regulation.bin");
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(path,reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(path);
        foreach(var f in check.Files.Where(f=>f.Name!=file.Name))if(!f.Bytes.SequenceEqual(others[f.Name]))throw new Exception($"Unrelated param changed: {f.Name}");
        var actual=PARAM.Read(check.Files.Single(f=>f.Name==file.Name).Bytes);actual.ApplyParamdef(def);
        var rows=actual.Rows.ToDictionary(r=>r.ID);
        foreach(var row in original.Rows)foreach(var cell in row.Cells) {
            var value=rows[row.ID][cell.Def.InternalName].Value;
            if(icons.TryGetValue(row.ID,out var icon)&&cell.Def.InternalName=="iconId") {if(Convert.ToInt32(value)!=icon)throw new Exception($"Talisman icon not written: {row.ID}");continue;}
            if(!System.Collections.StructuralComparisons.StructuralEqualityComparer.Equals(cell.Value,value))throw new Exception($"Existing accessory changed: {row.ID}/{cell.Def.InternalName}");
        }
        Console.WriteLine($"Talisman icons: {changed} of {icons.Count} talismans now use {icons.Values.Distinct().Count()} redrawn icons ({IconBase}-{icons.Values.Max()}); other params preserved");
    }
    public static void PatchMenu(string input,string output) {
        var icons=Required(input).Values.Distinct().Order().ToArray();
        int rows=(icons.Length+Columns-1)/Columns,width=Columns*160,height=rows*160;
        var pixels=new byte[width*height*4];
        for(int i=0;i<icons.Length;i++) {
            var rgba=File.ReadAllBytes(Path.Combine(input,"reward-icons",$"{icons[i]}.rgba"));
            if(rgba.Length!=102400)throw new Exception("Invalid talisman artwork");
            for(int y=0;y<160;y++)for(int x=0;x<160;x++) {
                int src=(y*160+x)*4,dst=((i/Columns*160+y)*width+i%Columns*160+x)*4;
                pixels[dst]=rgba[src+2];pixels[dst+1]=rgba[src+1];pixels[dst+2]=rgba[src];pixels[dst+3]=rgba[src+3];
            }
        }
        var dds=new byte[128+pixels.Length];
        void U(int offset,uint value)=>BitConverter.GetBytes(value).CopyTo(dds,offset);
        U(0,0x20534444);U(4,124);U(8,0x100f);U(12,(uint)height);U(16,(uint)width);U(20,(uint)width*4);U(28,1);U(76,32);U(80,0x41);U(88,32);U(92,0xff0000);U(96,0xff00);U(100,0xff);U(104,0xff000000);U(108,0x1000);pixels.CopyTo(dds,128);
        foreach(var quality in new[]{"hi","low"}) {
            var dir=Path.Combine(input,"menu",quality);var tpf=TPF.Read(Path.Combine(dir,"01_common.tpf.dcx"));var layout=BND4.Read(Path.Combine(dir,"01_common.sblytbnd.dcx"));
            foreach(var f in layout.Files.Where(f=>f.Name.EndsWith(".layout")&&!f.Name.EndsWith(Atlas+".layout"))) {
                var xml=XElement.Parse(Encoding.UTF8.GetString(f.Bytes));
                foreach(var e in xml.Elements())if(Regex.Match((string)e.Attribute("name")??"",@"MENU_ItemIcon_(\d+)") is var m&&m.Success&&icons.Contains(int.Parse(m.Groups[1].Value)))throw new Exception("Native icon ID collision");
            }
            var originalTextures=tpf.Textures.Where(t=>t.Name!=Atlas).ToArray();var originalLayouts=layout.Files.Where(f=>!f.Name.EndsWith(Atlas+".layout")).ToArray();
            tpf.Textures.RemoveAll(t=>t.Name==Atlas);layout.Files.RemoveAll(f=>f.Name.EndsWith(Atlas+".layout"));
            tpf.Textures.Add(new TPF.Texture(Atlas,9,0,dds,TPF.TPFPlatform.PC){Mipmaps=1});
            var xmlOut=new XElement("TextureAtlas",new XAttribute("imagePath",Atlas+".png"));
            for(int i=0;i<icons.Length;i++)xmlOut.Add(new XElement("SubTexture",new XAttribute("name",$"MENU_ItemIcon_{icons[i]}.png"),new XAttribute("x",i%Columns*160),new XAttribute("y",i/Columns*160),new XAttribute("width",160),new XAttribute("height",160),new XAttribute("half",0)));
            var template=layout.Files.First(f=>f.Name.EndsWith(".layout"));
            var name=template.Name[..(template.Name.LastIndexOf('\\')+1)]+Atlas+".layout";
            layout.Files.Add(new BinderFile(template.Flags,layout.Files.Max(f=>f.ID)+1,name,Encoding.UTF8.GetBytes(xmlOut.ToString())){CompressionInfo=template.CompressionInfo});
            var dest=Path.Combine(output,"menu",quality);Directory.CreateDirectory(dest);
            tpf.Write(Path.Combine(dest,"01_common.tpf.dcx"));layout.Write(Path.Combine(dest,"01_common.sblytbnd.dcx"));
            var check=TPF.Read(Path.Combine(dest,"01_common.tpf.dcx"));var checkLayout=BND4.Read(Path.Combine(dest,"01_common.sblytbnd.dcx"));
            foreach(var t in originalTextures)if(!check.Textures.Single(x=>x.Name==t.Name).Bytes.SequenceEqual(t.Bytes))throw new Exception("Unrelated texture changed");
            foreach(var f in originalLayouts)if(!checkLayout.Files.Single(x=>x.ID==f.ID).Bytes.SequenceEqual(f.Bytes))throw new Exception("Unrelated layout changed");
            if(!check.Textures.Single(t=>t.Name==Atlas).Bytes.SequenceEqual(dds))throw new Exception("Talisman atlas roundtrip failed");
            Console.WriteLine($"{quality}: {icons.Length} talisman icons added ({width}x{height}), existing resources preserved");
        }
    }
}
