using SoulsFormats;
using System.Text.Json;

static class PartyEnemyAudit {
    record Requirement(int Act,int Model,int Npc,int Think,int Copies,string Label,int Source);
    public static void Run(string output) {
        var requirements=File.ReadAllLines("outputs/erdtree-tower/scene-assets/enemy-pools.txt").Select(line=>{
            var p=line.Split('|');int model=int.Parse(p[2]);
            return new Requirement(int.Parse(p[0]),model,900000000+model*10000,model*10000,4,p[4],-1);
        }).DistinctBy(r=>(r.Act,r.Npc)).ToList();
        foreach(var boss in BossAssets.Parts("outputs/erdtree-tower/scene-assets"))
            foreach(bool elite in new[]{false,true})requirements.Add(new(boss.Act,boss.Model,BossAssets.Npc(boss,elite),boss.Think,1,boss.Label,boss.Source));
        var models=requirements.Select(r=>$"c{r.Model:D4}").ToHashSet();
        var templates=new List<(string Map,MSBE.Part.Enemy Part)>();
        foreach(var path in Directory.EnumerateFiles("work/boss-report/maps","*.msb.dcx")) {
            var map=MSBE.Read(path);
            templates.AddRange(map.Parts.Enemies.Where(p=>models.Contains(p.ModelName)).Select(p=>(Path.GetFileNameWithoutExtension(Path.GetFileNameWithoutExtension(path)),p)));
        }
        var rows=requirements.Select(r=>{
            var matches=templates.Where(t=>t.Part.ModelName==$"c{r.Model:D4}"&&
                (r.Source<0||r.Label.StartsWith("tower_champion_")||t.Part.NPCParamID==r.Source&&t.Part.ThinkParamID==r.Think)).ToArray();
            var exact=matches.FirstOrDefault(t=>t.Part.Name==r.Label);
            var selected=exact.Part!=null?exact:matches.FirstOrDefault();
            return new {r.Act,r.Model,r.Npc,r.Think,r.Copies,r.Label,Found=selected.Part!=null,
                SourceMap=selected.Map,SourcePart=selected.Part?.Name,SourceNpc=selected.Part?.NPCParamID,
                ExactParameters=selected.Part!=null&&selected.Part.NPCParamID==r.Source&&selected.Part.ThinkParamID==r.Think,
                Collision=selected.Part?.CollisionPartName,WalkRoute=selected.Part?.WalkRouteName,
                Activation=selected.Part?.ChrActivateCondParamID};
        }).ToArray();
        Directory.CreateDirectory(output);
        File.WriteAllText(Path.Combine(output,"party-enemy-templates.json"),JsonSerializer.Serialize(rows,new JsonSerializerOptions{WriteIndented=true}));
        foreach(var group in rows.GroupBy(r=>r.Act))Console.WriteLine($"Act {group.Key}: {group.Count()} variants, {group.Sum(r=>r.Copies)} slots, {group.Count(r=>!r.Found)} missing templates");
        foreach(var missing in rows.Where(r=>!r.Found))Console.WriteLine($"MISSING act={missing.Act} model={missing.Model} npc={missing.Npc} label={missing.Label}");
    }
}
