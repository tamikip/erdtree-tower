using SoulsFormats;
using System.Numerics;
var game=args[0];var output=args[1];var cache=args[2];
if(args.Contains("personal-events-test")){PersonalEvents.Test();return;}
if(args.Contains("party-enemy-events-test")){PartyEnemyEvents.Test();return;}
Oodle.Oodle6Ptr=System.Runtime.InteropServices.NativeLibrary.Load(Path.Combine(game,"oo2core_6_win64.dll"));
if(args.Contains("ruby-spells-export")){RubySpellAssets.Export(game,args[4],output);return;}
if(args.Contains("ruby-spells-patch")){RubySpellAssets.Patch(cache,output);return;}
if(args.Contains("affix-patch")){AffixAssets.Patch(cache,output);return;}
if(args.Contains("advanced-affix-patch")){AffixAssets.Patch(cache,output,true);return;}
if(args.Contains("advanced-affix-verify")){AdvancedAffixAssets.Verify(cache);return;}
if(args.Contains("game-version-rebase")){GameVersionAssets.Rebase(game,cache,output);return;}
if(args.Contains("archer-verify")){ArcherAssets.Verify(cache);return;}
if(args.Contains("archer-patch")){ArcherAssets.Patch(cache,output);return;}
if(args.Contains("party-guard-flags")){PartyGuardAssets.PatchFlags(cache,output);return;}
if(args.Contains("melina-patch")){MelinaAssets.Patch(cache,output);return;}
if(args.Contains("balance-patch")){BalanceAssets.Patch(cache,output);return;}
if(args.Contains("stormveil-build")){StormveilAssets.Build(output);RecoveryText.Patch(cache,output);return;}
if(args.Contains("chariot-build")){ChariotAssets.Build(output);RecoveryText.Patch(cache,output);return;}
if(args.Contains("arena-chariot-build")){ArenaChariotAssets.Build(cache,output);return;}
if(args.Contains("arena-chariot-loading")){ArenaChariotAssets.PatchLoading(cache,output);return;}
if(args.Contains("stormveil-inspect")) {
    var map=MSBE.Read("work/boss-report/maps/m10_00_00_00.msb.dcx");
    foreach(var r in map.Regions.GetEntries().Where(r=>r is MSBE.Region.SpawnPoint))Console.WriteLine($"SPAWN {r.Name} {r.EntityID} {r.Position} {r.Rotation}");
    foreach(var p in map.Parts.Players)Console.WriteLine($"PLAYER {p.Name} {p.EntityID} {p.Position} {p.Rotation}");
    foreach(var a in map.Parts.Assets.Where(a=>a.EntityID>0&&a.EntityID<uint.MaxValue))Console.WriteLine($"ASSET {a.Name} {a.ModelName} {a.EntityID} {a.Position} {a.Rotation}");
    var events=EMEVD.Read("work/scene-cache/m10_00_00_00.emevd.dcx");
    foreach(var e in events.Events.Where(e=>e.ID==0||e.Instructions.Any(i=>Enumerable.Range(0,Math.Max(0,i.ArgData.Length-3)).Any(n=>BitConverter.ToUInt32(i.ArgData,n)==10001500)))) {
        Console.WriteLine($"EVENT {e.ID}");
        foreach(var i in e.Instructions)Console.WriteLine($"{i.Bank}[{i.ID}] {Convert.ToHexString(i.ArgData)}");
    }
    return;
}
if(args.Contains("weapon-sale-patch")){WeaponSaleAssets.Patch(cache,output);return;}
if(args.Contains("melina-inspect")) {
    foreach(var path in Directory.EnumerateFiles("work/boss-report/maps","*.msb.dcx").Concat(Directory.EnumerateFiles("work/scene-cache","*.msb.dcx"))) {
        foreach(var e in MSBE.Read(path).Parts.Enemies.Where(e=>e.ModelName=="c2180"))Console.WriteLine($"{path}|{e.Name}|{e.NPCParamID}|{e.ThinkParamID}|{e.CharaInitID}|{e.BackupEventAnimID}");
    }
    return;
}
if(args.Contains("comet-inspect")){CometInspect.Run(game,output);return;}
if(args.Contains("recovery-text")){RecoveryText.Patch(cache,output);return;}
if(args.Contains("talisman-menu-text")){RecoveryText.PatchTalismans(cache,output);return;}
if(args.Contains("chest-text")){RecoveryText.Patch(cache,output,true);return;}
if(args.Contains("passive-patch")){PassiveAssets.Patch(cache,output);return;}
if(args.Contains("rune-audit")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(cache,"regulation.bin"));
    var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="SpEffectParam").Bytes);
    p.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/SpEffect.xml"));
    foreach(var row in p.Rows.Where(r=>Convert.ToSingle(r["soulRate"].Value)!=1))Console.WriteLine($"{row.ID}: soulRate={row["soulRate"].Value}");
    return;
}
if(args.Contains("contract-patch")){ContractAssets.PatchExpanded(cache,output);return;}
if(args.Contains("bandit-fix")){PassiveAssets.FixBandit(cache,output);return;}
if(args.Contains("contract-audit")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(cache,"regulation.bin"));
    PARAM Read(string name,string def) {var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{def}.xml"));return p;}
    var acc=Read("EquipParamAccessory","EquipParamAccessory");var effects=Read("SpEffectParam","SpEffect");
    foreach(int id in new[]{2040,2050,2090,2110}) {
        var item=acc.Rows.Single(r=>r.ID==id);int effect=Convert.ToInt32(item["refId"].Value);
        var row=effects.Rows.Single(r=>r.ID==effect);
        Console.WriteLine($"Accessory {id}: effect={effect} "+string.Join(" ",row.Cells.Where(c=>c.Def.InternalName.Contains("AttackRate")||c.Def.InternalName.Contains("Spot")||c.Def.InternalName.Contains("condition")||c.Def.InternalName.Contains("SubCategory")||c.Def.InternalName=="stateInfo").Select(c=>$"{c.Def.InternalName}={c.Value}")));
    }
    return;
}
if(args.Contains("bolus-audit")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(cache,"regulation.bin"));
    PARAM Read(string name,string def) {var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{def}.xml"));return p;}
    var goods=Read("EquipParamGoods","EquipParamGoods");var effects=Read("SpEffectParam","SpEffect");
    foreach(int id in new[]{900,910,920,930,940,950,960}) {
        var item=goods.Rows.Single(r=>r.ID==id);int effect=Convert.ToInt32(item["refId_default"].Value);
        var row=effects.Rows.Single(r=>r.ID==effect);
        Console.WriteLine($"Bolus {id}: category={item["refCategory"].Value} effect={effect} "+string.Join(" ",row.Cells.Where(c=>c.Def.InternalName.Contains("AttackPower")||c.Def.InternalName=="stateInfo"||c.Def.InternalName=="effectEndurance").Select(c=>$"{c.Def.InternalName}={c.Value}")));
    }
    return;
}
if(args.Contains("bgm-audit")){MusicAudit.Verify(cache);return;}
if(args.Contains("trial-stage")){TrialAssets.Build(cache,output);return;}
if(args.Contains("trial-verify")){TrialAssets.Verify(cache);return;}
if(args.Contains("descent-verify")){TrialAssets.VerifyDescent(cache);return;}
if(args.Contains("descent-patch")){TrialAssets.PatchDescent(cache,output);return;}
if(args.Contains("arrow-shop-inspect")){ArrowShopAssets.Inspect(cache);return;}
if(args.Contains("arrow-shop-stage")){ArrowShopAssets.Build(cache,output);return;}
if(args.Contains("resistance-shop-stage")){ResistanceShopAssets.Build(cache,output);return;}
if(args.Contains("hub-standing-patch")){PartyHubAssets.PatchStanding(cache,output);return;}
if(args.Contains("party-guard-stage")){PartyGuardAssets.Build(cache,output);return;}
if(args.Contains("party-guard-test")){PartyGuardAssets.Verify(cache);return;}
if(args.Contains("party-status-patch")){PartyStatusText.Patch(cache,output);return;}
if(args.Contains("personal-chest-test")){PersonalChestEvents.Test(cache);return;}
if(args.Contains("personal-chest-patch")){PersonalChestEvents.Patch(cache,output);return;}
if(args.Contains("party-completion-patch")){PartyCompletionEvents.Patch(cache,output);return;}
if(args.Contains("party-scarab-stage")){PartyScarabAssets.Build(cache,output);return;}
if(args.Contains("party-enemy-load-patch")){PartyEnemyEvents.Test();PartyEnemyEvents.PatchLoading(cache,output);return;}
if(args.Contains("personal-events-patch")){PersonalEvents.Patch(cache,output);return;}
if(args.Contains("party-hub-stage")){PartyHubAssets.Build(cache,output);return;}
if(args.Contains("party-camp-stage")){PartyCampAssets.Build(cache,output);return;}
if(args.Contains("party-service-stage")){PartyServiceAssets.Build(cache,output);return;}
if(args.Contains("party-placement-audit")){PartyPlacement.Audit(cache);return;}
if(args.Contains("party-model-audit")||args.Contains("party-model-patch")){PartyModels.Process(cache,output,args.Contains("party-model-patch"));return;}
if(args.Contains("party-service-load-test")){PartyServiceAssets.TestLoading();return;}
if(args.Contains("party-service-load-patch")) {
    PartyServiceAssets.TestLoading();
    foreach(var group in File.ReadAllLines("outputs/erdtree-tower/scene-assets/party-services.txt").Select(l=>l.Split('|')).GroupBy(r=>r[0])) {
        var path=Path.Combine(cache,"event",group.Key+".emevd.dcx");var file=EMEVD.Read(path);
        foreach(var row in group) {uint id=uint.Parse(row[2]);var ev=file.Events.Single(e=>e.ID==id);file.Events[file.Events.IndexOf(ev)]=PartyServiceAssets.Visibility(id);}
        var destination=Path.Combine(output,"event",group.Key+".emevd.dcx");Directory.CreateDirectory(Path.GetDirectoryName(destination)!);file.Write(destination);
        var check=EMEVD.Read(destination);
        foreach(var row in group) {uint id=uint.Parse(row[2]);var actual=check.Events.Single(e=>e.ID==id);PersonalEvents.Verify(actual);if(actual.Instructions.Count!=9)throw new Exception("Service load event roundtrip failed");}
    }
    return;
}
if(args.Contains("party-collision-inspect")) {
    var map=MSBE.Read(Path.Combine(cache,"map/mapstudio/m45_02_00_00.msb.dcx"));
    foreach(var p in map.Parts.Collisions)Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new{p.Name,p.ModelName,p.Unk1}));
    return;
}
if(args.Contains("party-service-inspect")) {
    foreach(var name in new[]{"m45_01_00_00","m11_10_00_00"}) {
        var map=MSBE.Read(Path.Combine(cache,"map/mapstudio",name+".msb.dcx"));
        foreach(var p in map.Parts.Enemies.Where(p=>p.EntityID is 45011800 or 45011801 or 45010100 or 45001800 or 45001801))
            Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(new{name,p.EntityID,p.ModelName,p.CollisionPartName,p.MapStudioLayer,p.GameEditionDisable,p.Unk1,p.TileLoad,p.Unk8,p.Gparam,Collision=map.Parts.Collisions.FirstOrDefault(c=>c.Name==p.CollisionPartName)?.Unk1,Model=map.Models.GetEntries().Single(m=>m.Name==p.ModelName)}));
    }
    return;
}
if(args.Contains("party-enemy-audit")){PartyEnemyAudit.Run(output);return;}
if(args.Contains("party-enemy-stage")){PartyEnemyAssets.Build(cache,output);return;}
if(args.Contains("party-param-audit")||args.Contains("party-param-stage")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(cache,"regulation.bin"));
    var npc=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="NpcParam").Bytes);
    npc.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/NpcParam.xml"));
    using var templates=System.Text.Json.JsonDocument.Parse(File.ReadAllText("work/party-entity-audit/party-enemy-templates.json"));
    var ids=templates.RootElement.EnumerateArray().Select(r=>r.GetProperty("Npc").GetInt32()).Distinct().ToArray();
    var rows=ids.Select(id=>npc.Rows.Single(r=>r.ID==id)).ToArray();
    var disabled=rows.Where(r=>Convert.ToInt32(r["disableRespawn"].Value)!=0).Select(r=>r.ID).ToArray();
    Directory.CreateDirectory(output);
    if(args.Contains("party-param-stage")) {
        if(Path.GetFullPath(cache)==Path.GetFullPath(output))throw new Exception("Staging must not overwrite input assets");
        foreach(var row in rows)row["disableRespawn"].Value=Convert.ChangeType(0,row["disableRespawn"].Value.GetType());
        reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="NpcParam").Bytes=npc.Write();
        SoulsFormats.Cryptography.RegulationDecryptor.EncryptERRegulation(Path.Combine(output,"regulation.bin"),reg);
        var check=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        foreach(var file in reg.Files)if(!file.Bytes.SequenceEqual(check.Files.Single(f=>f.Name==file.Name).Bytes))throw new Exception("Staged regulation roundtrip mismatch");
    }
    File.WriteAllText(Path.Combine(output,"party-param-audit.json"),System.Text.Json.JsonSerializer.Serialize(new {Checked=rows.Length,DisableRespawn=disabled},new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
    Console.WriteLine($"Party NPC params checked={rows.Length}, disableRespawn={disabled.Length}");return;
}
Directory.CreateDirectory(cache);
if(args.Contains("party-entity-inspect")) {
    foreach(var name in new[]{"m11_10_00_00","m60_42_38_00","m60_46_39_00","m45_00_00_00"}) {
        var map=MSBE.Read(Path.Combine(cache,name+".msb.dcx"));
        Console.WriteLine($"MAP {name}: enemies={map.Parts.Enemies.Count}");
        foreach(var enemy in map.Parts.Enemies.Where(e=>e.ModelName is "c2170" or "c0000" || name=="m45_00_00_00")) {
            var properties=enemy.GetType().GetProperties().Where(p=>p.CanRead&&p.GetIndexParameters().Length==0)
                .Where(p=>p.Name.Contains("ID")||p.Name.Contains("Name")||p.Name.Contains("Network")||p.Name is "Position" or "Rotation")
                .ToDictionary(p=>p.Name,p=>p.GetValue(enemy)?.ToString());
            Console.WriteLine(System.Text.Json.JsonSerializer.Serialize(properties));
        }
    }
    return;
}
if(args.Contains("scarab-inspect")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
    foreach(var name in new[]{"NpcParam","NpcThinkParam"}) {
        var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)==name).Bytes);
        p.ApplyParamdef(PARAMDEF.XmlDeserialize($"work/SoulsRandomizers/diste/Defs/{name}.xml"));
        foreach(var row in p.Rows.Where(r=>name=="NpcParam"?r.ID==41912000:r.ID>=41910000&&r.ID<41920000).Take(12))
            Console.WriteLine($"{name} {row.ID}: "+string.Join(" ",row.Cells.Where(c=>name=="NpcThinkParam"||new[]{"hp","getSoul","itemLotId_enemy","itemLotId_map","spEffectID0","spEffectID1","spEffectID2","spEffectID3","spEffectID4","spEffectID5","spEffectID6","spEffectID7"}.Contains(c.Def.InternalName)).Select(c=>$"{c.Def.InternalName}={c.Value}")));
    }
    return;
}
if(args.Contains("boss-export")){BossAssets.Export(output);return;}
if(args.Contains("battlefield-inspect")) {
    foreach(var (map,model) in new[]{("m10_00_00_00","c4750"),("m12_01_00_00","c4650"),("m11_00_00_00","c2130")}) {
        var msb=MSBE.Read(Path.Combine("work/boss-report/maps",map+".msb.dcx"));
        foreach(var r in msb.Regions.GetEntries().Where(r=>r.EntityID%10000>=2800&&r.EntityID%10000<3000))Console.WriteLine($"POINT {map} {r.EntityID} {r.Name} {r.Position} {r.Rotation}");
        foreach(var enemy in msb.Parts.Enemies.Where(e=>e.ModelName==model)) {
            Console.WriteLine($"{map} {enemy.Name} {enemy.EntityID} {enemy.Position} rot={enemy.Rotation} collision={enemy.CollisionPartName}");
            foreach(var r in msb.Regions.GetEntries().Where(r=>r.EntityID>0&&Vector3.Distance(r.Position,enemy.Position)<40).Take(25))Console.WriteLine($"REGION {r.Name} {r.EntityID} {r.Position} rot={r.Rotation}");
        }
    }
    return;
}
if(args.Contains("pool-inspect")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
    var npc=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="NpcParam").Bytes);
    var ai=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="NpcThinkParam").Bytes);
    foreach(var line in File.ReadAllLines("work/SoulsRandomizers/diste/Names/ModelName.txt")) {
        if(line.Length<5||line[0]!='c'||!int.TryParse(line.AsSpan(1,4),out int model)||model<2000||model>=5000)continue;
        if(npc.Rows.Any(r=>r.ID==model*10000)&&ai.Rows.Any(r=>r.ID==model*10000))Console.WriteLine(line);
    }
    return;
}
if(args.Contains("verify-native")){NativeAssets.Verify(output);BattlefieldAssets.Verify(output);return;}
if(args.Contains("battlefields")){BattlefieldAssets.Build(output);return;}
if(args.Contains("rarity-assets")){RewardAssets.BuildRarities(game,output);return;}
if(args.Contains("arena-inspect")) {
    foreach(var name in new[]{"m45_00_00_00","m45_01_00_00","m45_02_00_00"}) {
        var map=MSBE.Read(Path.Combine(cache,name+".msb.dcx"));
        foreach(var e in map.Parts.Enemies)Console.WriteLine($"{name} enemy={e.EntityID} model={e.ModelName} npc={e.NPCParamID} talk={e.TalkID} pos={e.Position}");
        foreach(var r in map.Regions.GetEntries().Where(r=>r.EntityID%10000==2600))Console.WriteLine($"{name} entry={r.EntityID} pos={r.Position}");
    }
    return;
}
if(args.Contains("catalog-assets")){CatalogAssets.Build(game,output,cache);return;}
if(args.Contains("portal-inspect")) {
    var common=EMEVD.Read(Path.Combine(cache,"common_func.emevd.dcx"));
    foreach(var ev in common.Events.Where(e=>e.Instructions.Any(i=>i.Bank==2006&&i.ID==4))) {
        Console.WriteLine("SFX EVENT "+ev.ID);
        foreach(var ins in ev.Instructions.Where(i=>i.Bank==2006 || i.Bank==3&&i.ID==24))Console.WriteLine($"{ins.Bank}[{ins.ID}] {Convert.ToHexString(ins.ArgData)}");
    }
    return;
}
var paths=new[]{"/map/mapstudio/m45_00_00_00.msb.dcx","/map/mapstudio/m45_01_00_00.msb.dcx","/map/mapstudio/m60_46_39_00.msb.dcx","/map/mapstudio/m10_00_00_00.msb.dcx","/event/m45_00_00_00.emevd.dcx","/event/m45_01_00_00.emevd.dcx","/event/m10_00_00_00.emevd.dcx","/event/common_func.emevd.dcx"};
paths=paths.Concat(new[]{"/map/mapstudio/m45_02_00_00.msb.dcx","/event/m45_02_00_00.emevd.dcx"}).ToArray();
paths=paths.Append("/event/m60_46_39_00.emevd.dcx").ToArray();
paths=paths.Concat(new[]{"/map/mapstudio/m11_10_00_00.msb.dcx","/event/m11_10_00_00.emevd.dcx"}).ToArray();
paths=paths.Concat(new[]{"/script/talk/m60_00_00_00.talkesdbnd.dcx","/script/talk/m00_00_00_00.talkesdbnd.dcx","/map/mapstudio/m60_42_38_00.msb.dcx","/msg/zhocn/menu.msgbnd.dcx","/msg/zhocn/item.msgbnd.dcx"}).ToArray();
paths=paths.Concat(new[]{"/msg/zhocn/menu_dlc02.msgbnd.dcx","/msg/zhocn/item_dlc02.msgbnd.dcx"}).ToArray();
if(args.Contains("reward-assets"))paths=paths.Concat(new[]{"/menu/hi/01_common.tpf.dcx","/menu/hi/01_common.sblytbnd.dcx"}).ToArray();
foreach(var archive in new[]{"Data0","Data1","Data2","Data3"}) {
    var missing=paths.Where(p=>!File.Exists(Path.Combine(cache,Path.GetFileName(p)))).ToArray();if(missing.Length==0)break;
    using var header=UXM.CryptographyUtility.DecryptRsa(Path.Combine(game,archive+".bhd"),UXM.ArchiveKeys.EldenRingKeys[archive]);
    var bhd=BHD5.Read(header,BHD5.Game.EldenRing);using var data=File.OpenRead(Path.Combine(game,archive+".bdt"));
    foreach(var path in missing) { ulong key=0;foreach(var ch in path)key=unchecked(key*0x85+ch);var file=bhd.Buckets.SelectMany(b=>b).FirstOrDefault(f=>f.FileNameHash==key);if(file!=null)File.WriteAllBytes(Path.Combine(cache,Path.GetFileName(path)),file.ReadFile(data)); }
}
if(args.Contains("reward-assets")){RewardAssets.Build(game,output,cache);return;}
if(args.Contains("level-inspect")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
    var p=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="CalcCorrectGraph").Bytes);
    p.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/CalcCorrectGraph.xml"));
    foreach(var r in p.Rows.Where(r=>Convert.ToSingle(r["init_inclination_soul"].Value)!=0||Convert.ToSingle(r["adjustment_value"].Value)!=0))Console.WriteLine($"GRAPH {r.ID} "+string.Join(" ",r.Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
    var shop=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam").Bytes);
    shop.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/ShopLineupParam.xml"));
    foreach(var row in shop.Rows.Where(r=>Convert.ToInt32(r["mtrlId"].Value)>=0).Take(10))Console.WriteLine($"BARTER {row.ID}: "+string.Join(" ",row.Cells.Where(c=>new[]{"equipType","value","mtrlId","costType","sellQuantity"}.Contains(c.Def.InternalName)).Select(c=>$"{c.Def.InternalName}={c.Value}")));
    return;
}
if(args.Contains("camp-inspect")) {
    foreach(var map in new[]{"m45_00_00_00","m45_01_00_00","m45_02_00_00"}) {
        var m=MSBE.Read(Path.Combine(cache,map+".msb.dcx"));
        foreach(var p in m.Parts.Players)Console.WriteLine($"PLAYER {map} {p.EntityID} {p.Name} {p.Position} {p.Rotation}");
        foreach(var r in m.Regions.GetEntries().Where(r=>r.EntityID>=45000000&&r.EntityID<45023000))Console.WriteLine($"REGION {map} {r.EntityID} {r.Position} {r.Rotation}");
    }
    foreach(var name in new[]{"m00_00_00_00","m60_00_00_00"})foreach(var file in BND4.Read(Path.Combine(cache,name+".talkesdbnd.dcx")).Files) {
        var esd=ESD.Read(file.Bytes);
        foreach(var group in esd.StateGroups)foreach(var state in group.Value) {
            var commands=state.Value.EntryCommands.Where(c=>c.CommandBank==1&&new[]{24,28,48,143}.Contains(c.CommandID));
            foreach(var c in commands) {
                Console.WriteLine($"MENU {file.Name} {group.Key}:{state.Key} {c.CommandID} {string.Join(" ",c.Arguments.Select(Convert.ToHexString))}");
                foreach(var cond in state.Value.Conditions) {
                    Console.WriteLine($"COND {Convert.ToHexString(cond.Evaluator)} -> {cond.TargetState}");
                    if(cond.TargetState is long target&&group.Value.TryGetValue(target,out var next))foreach(var follow in next.Conditions)Console.WriteLine($"NEXT {Convert.ToHexString(follow.Evaluator)}");
                }
            }
        }
    }
    return;
}
if(args.Contains("music-inspect")) {
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
    foreach(var f in reg.Files.Where(f=>f.Name.Contains("BgmBoss"))) {
        Console.WriteLine(f.Name);var p=PARAM.Read(f.Bytes);foreach(var r in p.Rows.Take(12))Console.WriteLine($"BGM {r.ID} {r.Name}");
    }
    foreach(var name in new[]{"m10_00_00_00","common_func"})foreach(var e in EMEVD.Read(Path.Combine(cache,name+".emevd.dcx")).Events)
        foreach(var ins in e.Instructions.Where(i=>i.Bank==2010&&i.ID==10).Take(3))Console.WriteLine($"MUSIC {name} {e.ID}: {Convert.ToHexString(ins.ArgData)}");
    return;
}
if(args.Contains("shop-inspect")) {
    var b=BND4.Read(Path.Combine(cache,"item_dlc02.msgbnd.dcx"));
    foreach(var f in b.Files.Where(f=>new[]{10,11}.Contains(f.ID)))
        foreach(var e in FMG.Read(f.Bytes).Entries.Where(e=>e.Text!=null&&e.ID<40000000&&System.Text.RegularExpressions.Regex.IsMatch(e.Text,f.ID==11?"杖|圣印|印记":"壶|肉干|肉块|肝|飞刀|虾|腌制|苔药")))Console.WriteLine($"{f.ID}|{e.ID}|{e.Text}");
    return;
}
if(args.Contains("reward-inspect")) {
    var b=BND4.Read(Path.Combine(cache,"item_dlc02.msgbnd.dcx"));
    foreach(var f in b.Files.Where(f=>new[]{10,12}.Contains(f.ID)))
        foreach(var e in FMG.Read(f.Bytes).Entries.Where(e=>e.Text!=null&&(e.Text.Contains("骑士")||e.Text.Contains("辉石")||e.Text.Contains("观星")||e.Text.Contains("预言")||e.Text.Contains("恢复")||e.Text.Contains("投火"))).Take(100))Console.WriteLine($"{f.ID}|{e.ID}|{e.Text}");
    return;
}
if(args.Contains("talisman-inspect")) {
    var b=BND4.Read(Path.Combine(cache,"item_dlc02.msgbnd.dcx"));
    var f=b.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="AccessoryName");
    Console.WriteLine($"ACCESSORY FMG {f.ID}");
    foreach(var e in FMG.Read(f.Bytes).Entries.Where(e=>!string.IsNullOrWhiteSpace(e.Text)))Console.WriteLine($"{e.ID}|{e.Text}");
    var reg=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(game,"regulation.bin"));
    var shop=PARAM.Read(reg.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam").Bytes);
    shop.ApplyParamdef(PARAMDEF.XmlDeserialize("work/SoulsRandomizers/diste/Defs/ShopLineupParam.xml"));
    foreach(var row in shop.Rows.Where(r=>new[]{0,2,3,4}.Contains(Convert.ToInt32(r["equipType"].Value))).GroupBy(r=>r["equipType"].Value).Select(g=>g.First()))
        Console.WriteLine("SHOP "+string.Join(" ",row.Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
    if(File.Exists(Path.Combine(output,"regulation.bin"))) {
        var built=SoulsFormats.Cryptography.RegulationDecryptor.DecryptERRegulation(Path.Combine(output,"regulation.bin"));
        var p=PARAM.Read(built.Files.Single(f=>Path.GetFileNameWithoutExtension(f.Name)=="ShopLineupParam").Bytes);p.ApplyParamdef(shop.AppliedParamdef);
        Console.WriteLine("BUILT "+string.Join(" ",p.Rows.Single(r=>r.ID==9500000).Cells.Select(c=>$"{c.Def.InternalName}={c.Value}")));
    }
    return;
}
if(args.Contains("text-inspect")) {
    var menu=BND4.Read(Path.Combine(cache,"menu_dlc02.msgbnd.dcx"));
    foreach(var f in menu.Files) {
        Console.WriteLine($"FMG {f.ID} {f.Name}");
        foreach(var e in FMG.Read(f.Bytes).Entries.Where(e=>e.Text is "离开" or "购买"))Console.WriteLine($"  {e.ID}: {e.Text}");
    }
    return;
}
if(args.Contains("native-inspect")) {
    foreach(var ev in EMEVD.Read(Path.Combine(cache,"m10_00_00_00.emevd.dcx")).Events)foreach(var ins in ev.Instructions.Where(i=>i.Bank==2003&&i.ID==72))Console.WriteLine($"BLOODSTAIN {Convert.ToHexString(ins.ArgData)}");
    var hub=MSBE.Read(Path.Combine(cache,"m11_10_00_00.msb.dcx"));
    foreach(var region in hub.Regions.GetEntries().Where(r=>r.EntityID>0))Console.WriteLine($"HUBREG {region.GetType().Name} {region.EntityID} {region.Position}");
    foreach(var npc in hub.Parts.Enemies.Where(n=>n.EntityID>0))Console.WriteLine($"HUBNPC {npc.ModelName} {npc.EntityID} {npc.Position}");
    foreach(var ev in EMEVD.Read(Path.Combine(cache,"m60_46_39_00.emevd.dcx")).Events.Where(e=>e.ID==1035472200))foreach(var i in ev.Instructions)Console.WriteLine($"PORTALDEF {ev.ID} {i.Bank}[{i.ID}] {Convert.ToHexString(i.ArgData)}");
    foreach(var ev in EMEVD.Read(Path.Combine(cache,"m60_46_39_00.emevd.dcx")).Events)foreach(var i in ev.Instructions)if(Convert.ToHexString(i.ArgData).Contains(Convert.ToHexString(BitConverter.GetBytes(1046391600u))))Console.WriteLine($"PORTAL {ev.ID} {i.Bank}[{i.ID}] {Convert.ToHexString(i.ArgData)}");
    var talk=BND4.Read(Path.Combine(cache,"m60_00_00_00.talkesdbnd.dcx"));
    foreach(var f in talk.Files.Where(f=>f.Name.Contains("800006"))) {
        Console.WriteLine($"TALK {f.ID} {f.Name}");
        var esd=ESD.Read(f.Bytes);var dump=esd.StateGroups.ToDictionary(g=>g.Key,g=>g.Value.ToDictionary(s=>s.Key,s=>new{entry=s.Value.EntryCommands.Select(c=>new{c.CommandBank,c.CommandID,args=c.Arguments.Select(Convert.ToHexString)}),conditions=s.Value.Conditions.Select(c=>new{c.TargetState,expr=Convert.ToHexString(c.Evaluator)})}));
        File.WriteAllText(Path.Combine(cache,"merchant-esd.json"),System.Text.Json.JsonSerializer.Serialize(dump,new System.Text.Json.JsonSerializerOptions{WriteIndented=true}));
    }
    var world=MSBE.Read(Path.Combine(cache,"m60_42_38_00.msb.dcx"));
    foreach(var n in world.Parts.Enemies.Where(n=>n.ModelName=="c3200"))Console.WriteLine($"NPC {n.Name} npc={n.NPCParamID} think={n.ThinkParamID} talk={n.TalkID}");
    foreach(var name in new[]{"menu","item"}) {
        var bnd=BND4.Read(Path.Combine(cache,name+".msgbnd.dcx"));
        foreach(var f in bnd.Files.Where(f=>name=="item"&&new[]{10,11,35}.Contains(f.ID)))foreach(var e in FMG.Read(f.Bytes).Entries.Where(e=>e.Text!=null&&System.Text.RegularExpressions.Regex.IsMatch(e.Text,"双|爪|战灰：|恢复|火焰啊|投火|兽石|雷电枪")))Console.WriteLine($"ITEM {f.ID} {e.ID} {e.Text}");
    }
    return;
}
if(args.Contains("inspect")) {
    var eventMap=EMEVD.Read(Path.Combine(cache,"m10_00_00_00.emevd.dcx"));
    foreach(var ev in eventMap.Events)foreach(var ins in ev.Instructions) {
        var words=Enumerable.Range(0,ins.ArgData.Length/4).Select(i=>BitConverter.ToUInt32(ins.ArgData,i*4)).ToArray();
        if(words.Contains(10001250u))Console.WriteLine($"CHEST EVENT {ev.ID}: {ins.Bank}[{ins.ID}] {string.Join(",",words)}");
    }
    var common=EMEVD.Read(Path.Combine(cache,"common_func.emevd.dcx"));
    foreach(var ev in common.Events.Where(e=>e.ID>=90005500 && e.ID<=90005520)) {
        Console.WriteLine("COMMON "+ev.ID);foreach(var ins in ev.Instructions)Console.WriteLine($"{ins.Bank}[{ins.ID}] {Convert.ToHexString(ins.ArgData)}");
    }
    return;
}
NativeAssets.Build(game,output,cache);
var hubEvents=EMEVD.Read(Path.Combine(cache,"m11_10_00_00.emevd.dcx"));
var resetEvent=new EMEVD.Event(11100980,EMEVD.Event.RestBehaviorType.Restart);
if(hubEvents.Events.Any(e=>e.ID==resetEvent.ID))throw new Exception("Hub reset event conflict");
resetEvent.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,45000880u}));
resetEvent.Instructions.Add(new EMEVD.Instruction(2003,72,new object[]{1f,-1}));
resetEvent.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,45000880u,(byte)0}));
resetEvent.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
hubEvents.Events.Add(resetEvent);
hubEvents.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,(uint)resetEvent.ID,0u}));
Directory.CreateDirectory(Path.Combine(output,"event"));
hubEvents.Write(Path.Combine(output,"event","m11_10_00_00.emevd.dcx"));
var portalSource=MSBE.Read(Path.Combine(cache,"m60_46_39_00.msb.dcx"));
var chestSource=MSBE.Read(Path.Combine(cache,"m10_00_00_00.msb.dcx"));
foreach(var mapName in new[]{"m45_00_00_00","m45_01_00_00","m45_02_00_00"}) {
    var msb=MSBE.Read(Path.Combine(cache,mapName+".msb.dcx"));
    bool chest=mapName=="m45_01_00_00";bool shop=mapName=="m45_02_00_00";
    uint basis=shop?45020000u:chest?45010000u:45000000u;uint ready=basis+991;uint request=basis+992;
    msb.Parts.Enemies.Single(e=>e.EntityID==basis+100).TalkID=2000;
    foreach(var id in shop?new[]{basis+2600,basis+2710}:new[]{basis+2600}) {
        var entry=msb.Regions.GetEntries().Single(r=>r.EntityID==id);
        entry.Position=new Vector3(0,0.15f,10);entry.Rotation=Vector3.Zero;
    }
    foreach(var player in msb.Parts.Players) {player.Position=new Vector3(0,0.15f,10);player.Rotation=Vector3.Zero;}
    AddAsset(msb,portalSource,portalSource.Parts.Assets.Single(a=>a.EntityID==1046391600),basis+1900,new Vector3(0,0.03f,-12),new Vector3(0,180,0));
    if(chest)AddAsset(msb,chestSource,chestSource.Parts.Assets.Single(a=>a.EntityID==10001250),45011901,new Vector3(0,0.03f,2),new Vector3(0,180,0));
    if(chest)for(uint i=0;i<3;i++)AddAsset(msb,chestSource,chestSource.Parts.Assets.Single(a=>a.EntityID==10001250),45011910+i,new Vector3(((int)i-1)*5,0.03f,0),new Vector3(0,180,0));
    if(chest)RegionalAssets.BlastRegions(msb);
    var mapDir=Path.Combine(output,"map","mapstudio");Directory.CreateDirectory(mapDir);
    var mapPath=Path.Combine(mapDir,mapName+".msb.dcx");msb.Write(mapPath);
    var reread=MSBE.Read(mapPath);if(!reread.Parts.Assets.Any(a=>a.EntityID==basis+1900))throw new Exception("Map roundtrip failed");
    if(reread.Parts.Enemies.Single(e=>e.EntityID==basis+100).TalkID!=2000)throw new Exception("Spirit stone still requires native arena match state");
    var emevd=EMEVD.Read(Path.Combine(cache,mapName+".emevd.dcx"));
    if(chest)RegionalAssets.Build(emevd);
    long eventId=basis+980;
    if(emevd.Events.Any(e=>e.ID==eventId))throw new Exception("Event ID already occupied");
    var ev=new EMEVD.Event(eventId,EMEVD.Event.RestBehaviorType.Restart);
    PersonalEvents.Local(ev);
    ev.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,request}));
    ev.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,ready,(byte)0}));
    ev.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,request,(byte)0}));
    if(chest) {
        ev.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,45010990u,(byte)0}));
        ev.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,45010989u,(byte)0}));
        ev.Instructions.Add(new EMEVD.Instruction(2005,7,new object[]{45011901u,0}));
    }
    ev.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,ready,(byte)1}));
    ev.Instructions.Add(new EMEVD.Instruction(1001,0,new object[]{0.1f}));
    ev.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
    if(chest) {
        var animation=PersonalChestEvents.Create();
        emevd.Events.Add(animation);
        emevd.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,(uint)animation.ID,0u}));
    }
    var exit=new EMEVD.Event(eventId+2,EMEVD.Event.RestBehaviorType.Restart);
    PersonalEvents.Local(exit);
    exit.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,basis+993}));
    if(chest)exit.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,basis+995}));
    exit.Instructions.Add(new EMEVD.Instruction(3,24,new object[]{(sbyte)1,9500000,basis+1900}));
    exit.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
    exit.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,basis+994,(byte)1}));
    exit.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,basis+993}));
    exit.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
    emevd.Events.Add(exit);
    {
        var combatExit=new EMEVD.Event(eventId+3,EMEVD.Event.RestBehaviorType.Restart);
        PersonalEvents.Local(combatExit);
        combatExit.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,45020970u}));
        combatExit.Instructions.Add(new EMEVD.Instruction(3,24,new object[]{(sbyte)1,9500000,basis+1900}));
        combatExit.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)1}));
        combatExit.Instructions.Add(new EMEVD.Instruction(2003,66,new object[]{(byte)0,45020971u,(byte)1}));
        combatExit.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,45020970u}));
        combatExit.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
        var glow=PartyCompletionEvents.Banner(eventId+4);
        var visual=new EMEVD.Event(eventId+6,EMEVD.Event.RestBehaviorType.Restart);
        PersonalEvents.Local(visual);
        visual.Instructions.Add(new EMEVD.Instruction(2006,5,new object[]{basis+1900,1}));
        visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,basis+993}));
        if(chest)visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)1,(byte)1,(byte)0,basis+995}));
        visual.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)-1,(byte)1,(sbyte)1}));
        visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)-1,(byte)1,(byte)0,45020970u}));
        visual.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)-1}));
        visual.Instructions.Add(new EMEVD.Instruction(2006,4,new object[]{basis+1900,200,0xC4FD6}));
        visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)-2,(byte)0,(byte)0,basis+993}));
        if(chest)visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)-2,(byte)0,(byte)0,basis+995}));
        visual.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)2,(byte)1,(sbyte)-2}));
        visual.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)2,(byte)0,(byte)0,45020970u}));
        visual.Instructions.Add(new EMEVD.Instruction(0,0,new object[]{(sbyte)0,(byte)1,(sbyte)2}));
        visual.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
        var music=new EMEVD.Event(eventId+5,EMEVD.Event.RestBehaviorType.Restart);
        PersonalEvents.Local(music);
        music.Instructions.Add(new EMEVD.Instruction(2010,10,new object[]{203000,-1}));
        music.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)1,(byte)0,45020969u}));
        music.Instructions.Add(new EMEVD.Instruction(2010,10,new object[]{203000,0}));
        music.Instructions.Add(new EMEVD.Instruction(3,0,new object[]{(sbyte)0,(byte)0,(byte)0,45020969u}));
        music.Instructions.Add(new EMEVD.Instruction(2010,10,new object[]{203000,-1}));
        music.Instructions.Add(new EMEVD.Instruction(1000,4,new object[]{(byte)1}));
        foreach(var extra in new[]{combatExit,glow,music,visual}) {
            emevd.Events.Add(extra);
            emevd.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,(uint)extra.ID,0u}));
        }
    }
    emevd.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,(uint)exit.ID,0u}));
    emevd.Events.Add(ev);
    emevd.Events.Single(e=>e.ID==0).Instructions.Insert(0,new EMEVD.Instruction(2000,0,new object[]{0,(uint)eventId,0u}));
    BossAssets.Events(emevd,basis,output);
    foreach(var id in chest?new[]{eventId,eventId+1,eventId+2,eventId+3,eventId+5,eventId+6}:new[]{eventId,eventId+2,eventId+3,eventId+5,eventId+6})PersonalEvents.Verify(emevd.Events.Single(e=>e.ID==id));
    var eventDir=Path.Combine(output,"event");Directory.CreateDirectory(eventDir);var eventPath=Path.Combine(eventDir,mapName+".emevd.dcx");emevd.Write(eventPath);
    if(!EMEVD.Read(eventPath).Events.Any(e=>e.ID==eventId))throw new Exception("Event roundtrip failed");
    foreach(var instruction in emevd.Events.Where(e=>e.ID>=eventId && e.ID<=eventId+4).SelectMany(e=>e.Instructions)) {
        int expected=(instruction.Bank,instruction.ID) switch{(2000,2)=>4,(2007,2)=>4,(0,0)=>4,(2006,4)=>12,(2006,5)=>8,(3,0)=>8,(3,24)=>12,(2003,66)=>12,(2005,7)=>8,(2003,18)=>16,(2004,14)=>16,(2003,4)=>4,(1001,0)=>4,(1000,4)=>4,_=>throw new Exception("Unexpected instruction")};
        if(instruction.ArgData.Length!=expected)throw new Exception("Instruction layout mismatch");
    }
    Console.WriteLine($"Built {mapName}: portal, {(chest?"chest":"camp/merchant hall")}, handshake {ready}");
}
BattlefieldAssets.Build(output);
PartyHubAssets.Build(cache,output);
PartyCampAssets.Build(output,output);
PartyServiceAssets.Build(output,output);
PartyScarabAssets.Build(output,output);
MelinaAssets.Patch(output,output);
StormveilAssets.Build(output);
ChariotAssets.Build(output);
static void AddAsset(MSBE map,MSBE source,MSBE.Part.Asset donor,uint id,Vector3 pos,Vector3 rot) {
    if(map.Parts.GetEntries().Any(p=>p.EntityID==id))throw new Exception($"Entity ID {id} already occupied");
    var part=(MSBE.Part.Asset)donor.DeepCopy();part.Name=donor.ModelName+"_twr_"+id;part.EntityID=id;part.Position=pos;part.Rotation=rot;
    part.EntityGroupIDs=new uint[8];part.UnkPartNames=new string[6];part.UnkT54PartName=donor.UnkT54PartName==donor.Name?part.Name:null;
    part.Unk1.DisplayGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();part.Unk1.DrawGroups=Enumerable.Repeat(uint.MaxValue,8).ToArray();
    if(!map.Models.Assets.Any(m=>m.Name==part.ModelName))map.Models.Assets.Add((MSBE.Model.Asset)source.Models.Assets.Single(m=>m.Name==part.ModelName).DeepCopy());
    map.Parts.Assets.Add(part);
}
