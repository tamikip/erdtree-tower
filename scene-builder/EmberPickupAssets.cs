using System.Text;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using SoulsFormats;

static class EmberPickupAssets {
    const int GoodsBase=9900000,IconBase=65000;
    const string Atlas="SB_Icon_TowerEmbers";
    public static void Build(string input,string output) {
        var source=File.ReadAllText("src/embers.rs");
        var order=Regex.Match(source,@"pub const ALL:[^=]+?=\[(.*?)\];",RegexOptions.Singleline).Groups[1].Value;
        var keys=Regex.Matches(order,@"Self::(\w+)").Select(m=>m.Groups[1].Value).ToArray();
        var names=Regex.Matches(source,@"Self::(\w+)=>""([^""]+)""").GroupBy(m=>m.Groups[1].Value).ToDictionary(g=>g.Key,g=>g.First().Groups[2].Value);
        if(keys.Length!=37||keys.Distinct().Count()!=37||keys.Any(k=>!names.ContainsKey(k)))throw new Exception("Ember catalogue mismatch");
        Directory.CreateDirectory(output);
        var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(input,"regulation.bin"));
        var file=reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="EquipParamGoods");
        var goods=PARAM.Read(file.Bytes);
        var defs=Environment.GetEnvironmentVariable("TOWER_PARAMDEFS")??"../../work/SoulsRandomizers/diste/Defs";
        var def=PARAMDEF.XmlDeserialize(Path.Combine(defs,"EquipParamGoods.xml"));goods.ApplyParamdef(def);
        var baseline=goods.Write();
        void Set(PARAM.Row row,string key,int value)=>row[key].Value=Convert.ChangeType(value,row[key].Value.GetType());
        for(int i=0;i<keys.Length;i++) {
            var old=goods.Rows.SingleOrDefault(r=>r.ID==GoodsBase+i);
            if(old!=null&&old.Name!="Tower ember display "+keys[i])throw new Exception("Goods ID collision");
            if(old!=null)goods.Rows.Remove(old);
            var row=new PARAM.Row(goods.Rows.Single(r=>r.ID==10000)){ID=GoodsBase+i,Name="Tower ember display "+keys[i]};
            foreach(var (key,value) in new[]{("iconId",IconBase+i),("refId_default",-1),("refId_1",-1),("showDialogCondType",1),("showLogCondType",0),("isEquip",0),("enable_live",0),("isDrop",0)})Set(row,key,value);
            goods.Rows.Add(row);
        }
        goods.Rows.Sort((a,b)=>a.ID.CompareTo(b.ID));file.Bytes=goods.Write();
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),reg);
        var checkReg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        foreach(var original in reg.Files)if(!original.Bytes.SequenceEqual(checkReg.Files.Single(f=>f.Name==original.Name).Bytes))throw new Exception("Regulation roundtrip mismatch");
        var checkGoods=PARAM.Read(checkReg.Files.Single(f=>f.Name==file.Name).Bytes);checkGoods.ApplyParamdef(def);
        for(int i=0;i<keys.Length;i++){var r=checkGoods.Rows.Single(r=>r.ID==GoodsBase+i);if(Convert.ToInt32(r["iconId"].Value)!=IconBase+i||Convert.ToInt32(r["showDialogCondType"].Value)!=1)throw new Exception("Invalid display goods");}
        checkGoods.Rows.RemoveAll(r=>r.ID>=GoodsBase&&r.ID<GoodsBase+keys.Length);
        if(!checkGoods.Write().SequenceEqual(baseline))throw new Exception("Unrelated goods changed");
        foreach(var quality in new[]{"hi","low"}) {
            var dir=Path.Combine(input,"menu",quality);var tpf=TPF.Read(Path.Combine(dir,"01_common.tpf.dcx"));var layout=BND4.Read(Path.Combine(dir,"01_common.sblytbnd.dcx"));
            foreach(var f in layout.Files.Where(f=>f.Name.EndsWith(".layout")&&!f.Name.EndsWith(Atlas+".layout"))) {
                var xml=XElement.Parse(Encoding.UTF8.GetString(f.Bytes));
                foreach(var e in xml.Elements())if(Regex.Match((string)e.Attribute("name")??"",@"MENU_ItemIcon_(\d+)") is var m&&m.Success&&int.Parse(m.Groups[1].Value)>=IconBase&&int.Parse(m.Groups[1].Value)<IconBase+keys.Length)throw new Exception("Native icon ID collision");
            }
            var originalTextures=tpf.Textures.Where(t=>t.Name!=Atlas).ToArray();var originalLayouts=layout.Files.Where(f=>!f.Name.EndsWith(Atlas+".layout")).ToArray();
            tpf.Textures.RemoveAll(t=>t.Name==Atlas);layout.Files.RemoveAll(f=>f.Name.EndsWith(Atlas+".layout"));
            var pixels=new byte[1280*640*4];
            for(int i=0;i<keys.Length;i++) {
                var rgba=File.ReadAllBytes(Path.Combine(input,"reward-icons",$"{990000+i}.rgba"));
                if(rgba.Length!=102400)throw new Exception("Invalid ember artwork");
                for(int y=0;y<160;y++)for(int x=0;x<160;x++) {
                    int src=(y*160+x)*4,dst=((i/8*160+y)*1280+i%8*160+x)*4;
                    pixels[dst]=rgba[src+2];pixels[dst+1]=rgba[src+1];pixels[dst+2]=rgba[src];pixels[dst+3]=rgba[src+3];
                }
            }
            var dds=new byte[128+pixels.Length];
            void U(int offset,uint value)=>BitConverter.GetBytes(value).CopyTo(dds,offset);
            U(0,0x20534444);U(4,124);U(8,0x100f);U(12,640);U(16,1280);U(20,5120);U(28,1);U(76,32);U(80,0x41);U(88,32);U(92,0xff0000);U(96,0xff00);U(100,0xff);U(104,0xff000000);U(108,0x1000);pixels.CopyTo(dds,128);
            tpf.Textures.Add(new TPF.Texture(Atlas,9,0,dds,TPF.TPFPlatform.PC){Mipmaps=1});
            var xmlOut=new XElement("TextureAtlas",new XAttribute("imagePath",Atlas+".png"));
            for(int i=0;i<keys.Length;i++)xmlOut.Add(new XElement("SubTexture",new XAttribute("name",$"MENU_ItemIcon_{IconBase+i}.png"),new XAttribute("x",i%8*160),new XAttribute("y",i/8*160),new XAttribute("width",160),new XAttribute("height",160),new XAttribute("half",0)));
            var template=layout.Files.First(f=>f.Name.EndsWith(".layout"));
            var name=template.Name[..(template.Name.LastIndexOf('\\')+1)]+Atlas+".layout";
            layout.Files.Add(new BinderFile(template.Flags,layout.Files.Max(f=>f.ID)+1,name,Encoding.UTF8.GetBytes(xmlOut.ToString())){CompressionInfo=template.CompressionInfo});
            var dest=Path.Combine(output,"menu",quality);Directory.CreateDirectory(dest);
            tpf.Write(Path.Combine(dest,"01_common.tpf.dcx"));layout.Write(Path.Combine(dest,"01_common.sblytbnd.dcx"));
            var check=TPF.Read(Path.Combine(dest,"01_common.tpf.dcx"));var checkLayout=BND4.Read(Path.Combine(dest,"01_common.sblytbnd.dcx"));
            foreach(var t in originalTextures)if(!check.Textures.Single(x=>x.Name==t.Name).Bytes.SequenceEqual(t.Bytes))throw new Exception("Unrelated texture changed");
            foreach(var f in originalLayouts)if(!checkLayout.Files.Single(x=>x.ID==f.ID).Bytes.SequenceEqual(f.Bytes))throw new Exception("Unrelated layout changed");
            if(!check.Textures.Single(t=>t.Name==Atlas).Bytes.SequenceEqual(dds))throw new Exception("Ember atlas roundtrip failed");
            Console.WriteLine($"{quality}: 26 native icons added, existing resources preserved");
        }
        var msgPath="msg/zhocn/item_dlc02.msgbnd.dcx";var msg=BND4.Read(Path.Combine(input,msgPath));
        foreach(var f in msg.Files.Where(f=>Path.GetFileNameWithoutExtension(f.Name).Split("_dlc")[0]=="GoodsName")) {
            var fmg=FMG.Read(f.Bytes);
            for(int i=0;i<keys.Length;i++){var entry=fmg.Entries.SingleOrDefault(e=>e.ID==GoodsBase+i);if(entry!=null&&entry.Text!="余烬："+names[keys[i]])throw new Exception("FMG ID collision");if(entry!=null)fmg.Entries.Remove(entry);fmg.Entries.Add(new FMG.Entry(GoodsBase+i,"余烬："+names[keys[i]]));}
            f.Bytes=fmg.Write();
        }
        var tables=ItemTextTables.Read(msg);
        for(int i=0;i<keys.Length;i++)if(tables["GoodsName"].Entries.Single(e=>e.ID==GoodsBase+i).Text!="余烬："+names[keys[i]])throw new Exception("Ember name missing");
        Directory.CreateDirectory(Path.GetDirectoryName(Path.Combine(output,msgPath))!);msg.Write(Path.Combine(output,msgPath));
        Console.WriteLine("26 display-only goods and names generated; no inventory grant configured");
    }
}
