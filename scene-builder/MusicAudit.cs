using SoulsFormats;

static class MusicAudit {
    public static void Verify(string input) {
        for(int area=0;area<3;area++) {
            var file=EMEVD.Read(Path.Combine(input,"event",$"m45_0{area}_00_00.emevd.dcx"));
            long id=45000985+area*10000;
            var music=file.Events.Single(e=>e.ID==id);PersonalEvents.Verify(music);
            var starts=file.Events.Single(e=>e.ID==0).Instructions.Count(i=>i.Bank==2000&&i.ID==0&&i.ArgData.Length>=8&&BitConverter.ToUInt32(i.ArgData,4)==id);
            if(starts!=1)throw new Exception($"Music initialization mismatch: {id}");
            var sounds=music.Instructions.Where(i=>i.Bank==2010&&i.ID==10).ToArray();
            if(sounds.Length!=3||sounds.Any(i=>BitConverter.ToInt32(i.ArgData,0)!=203000)||!sounds.Select(i=>BitConverter.ToInt32(i.ArgData,4)).SequenceEqual(new[]{-1,0,-1}))throw new Exception("Music lifecycle mismatch");
            var waits=music.Instructions.Where(i=>i.Bank==3&&i.ID==0).ToArray();
            if(waits.Length!=2||waits.Any(i=>BitConverter.ToUInt32(i.ArgData,4)!=45020969)||waits[0].ArgData[1]!=1||waits[1].ArgData[1]!=0)throw new Exception("Music flag mismatch");
        }
        Console.WriteLine("Verified three arena BGM constructors, local playback and start/stop gates");
        MusicAssets.Verify(input);
    }
}
