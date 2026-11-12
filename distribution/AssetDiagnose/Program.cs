using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Runtime.Serialization.Json;
using System.Text;
using Tower.Assets;

static class Program {
    [StructLayout(LayoutKind.Sequential)] struct Memory {
        public uint Length, Load;
        public ulong Physical, AvailablePhysical, CommitLimit, AvailableCommit, Virtual, AvailableVirtual, ExtendedVirtual;
    }
    [DllImport("kernel32", SetLastError=true)] static extern bool GlobalMemoryStatusEx(ref Memory info);
    [DllImport("kernel32", CharSet=CharSet.Unicode, SetLastError=true)] static extern IntPtr LoadLibraryEx(string path,IntPtr file,uint flags);
    [DllImport("oo2core_6_win64.dll")] static extern long OodleLZ_GetDecodeBufferSize(long size,bool corruptionPossible);
    [DllImport("oo2core_6_win64.dll")] static extern long OodleLZ_Decompress(byte[] input,long inputLength,byte[] output,long outputLength,int safe,int crc,int verbosity,IntPtr baseBuffer,long bufferSize,IntPtr callback,IntPtr user,IntPtr memory,long memorySize,int phase);
    static void MemoryReport(string stage) {
        var m=new Memory{Length=(uint)Marshal.SizeOf(typeof(Memory))};
        if(GlobalMemoryStatusEx(ref m)) Console.WriteLine($"MEMORY stage={stage} load_percent={m.Load} physical_total={m.Physical} physical_available={m.AvailablePhysical} commit_limit={m.CommitLimit} commit_available={m.AvailableCommit} virtual_available={m.AvailableVirtual} private_bytes={Process.GetCurrentProcess().PrivateMemorySize64}");
    }
    static int Big(byte[] b,int i)=>checked((int)((uint)b[i]<<24|(uint)b[i+1]<<16|(uint)b[i+2]<<8|b[i+3]));
    static void ModuleReport() {
        foreach(ProcessModule module in Process.GetCurrentProcess().Modules)
            if(module.ModuleName.StartsWith("oo2core",StringComparison.OrdinalIgnoreCase))Console.WriteLine($"LOADED_LIBRARY path={module.FileName} sha256={AssetHash.FileHash(module.FileName)}");
    }
    static int Main(string[] args) {
        Console.OutputEncoding=new UTF8Encoding(false);
        try {
            if((args.Length!=2&&args.Length!=3)||!new[]{"original","trace"}.Contains(args[1]))throw new ArgumentException("Usage: Tower.AssetDiagnose <launcher-folder> <original|trace> [game-folder]");
            string root=Path.GetFullPath(args[0]),mode=args[1];
            Console.WriteLine($"DIAGNOSTIC mode={mode} utc={DateTime.UtcNow:o} os={Environment.OSVersion} x64={Environment.Is64BitProcess} clr={Environment.Version} cpu_count={Environment.ProcessorCount}");
            MemoryReport("start");
            string game=(args.Length==3?args[2]:File.ReadAllText(Path.Combine(root,"GamePath.txt"))).Trim().Trim('"');
            Console.WriteLine("GAME_DIRECTORY="+game);
            string manifestPath=Path.Combine(root,"asset-patches/manifest.json"),original=Path.Combine(root,"Tower.AssetRestore.exe");
            Console.WriteLine($"MANIFEST sha256={AssetHash.FileHash(manifestPath)}");
            Console.WriteLine($"ORIGINAL_EXE sha256={AssetHash.FileHash(original)} size={new FileInfo(original).Length}");
            AssetManifest manifest;
            using(var stream=File.OpenRead(manifestPath))manifest=(AssetManifest)new DataContractJsonSerializer(typeof(AssetManifest)).ReadObject(stream);
            var file=manifest.Files.Single(f=>f.Path=="menu/hi/01_common.tpf.dcx");
            if(!new[]{"Data0.bdt","Data1.bdt","Data2.bdt","Data3.bdt","DLC.bdt"}.Contains(file.Archive)||file.SourceSize<=0||file.SourceSize>512*1024*1024||file.Offset<0)throw new InvalidDataException("Invalid source recipe");
            string library=Path.Combine(game,"oo2core_6_win64.dll");
            string libraryHash=AssetHash.FileHash(library);
            Console.WriteLine($"LIBRARY expected={manifest.OodleHash} actual={libraryHash}");
            if(libraryHash!=manifest.OodleHash)throw new InvalidDataException("LIBRARY_HASH_MISMATCH");
            var source=new byte[file.SourceSize];
            using(var stream=File.OpenRead(Path.Combine(game,file.Archive))) {
                if(file.Offset>stream.Length-source.Length)throw new InvalidDataException("SOURCE_RANGE_INVALID");
                stream.Position=file.Offset;
                for(int offset=0;offset<source.Length;){int count=stream.Read(source,offset,source.Length-offset);if(count==0)throw new EndOfStreamException();offset+=count;}
            }
            var hash=AssetHash.Of(source);Console.WriteLine($"SOURCE expected={file.SourceHash} actual={hash}");
            if(hash!=file.SourceHash)throw new InvalidDataException("SOURCE_HASH_MISMATCH");
            foreach(var repair in file.Repairs){var b=Convert.FromBase64String(repair.Data);if(repair.Offset<0||repair.Offset>source.Length-b.Length)throw new InvalidDataException("Invalid repair");Buffer.BlockCopy(b,0,source,repair.Offset,b.Length);}
            Console.WriteLine($"PATCHED_SOURCE sha256={AssetHash.Of(source)} size={source.Length}");
            MemoryReport("before_decode");
            byte[] decoded;
            if(mode=="original") {
                var type=Assembly.LoadFile(original).GetType("Tower.Assets.OodleDcx",true);
                type.GetMethod("Load").Invoke(null,new object[]{game,manifest.OodleHash});ModuleReport();
                decoded=(byte[])type.GetMethod("Decode").Invoke(null,new object[]{source});
            } else {
                if(LoadLibraryEx(library,IntPtr.Zero,0x100|0x800)==IntPtr.Zero)throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
                ModuleReport();
                if(source.Length<76||Encoding.ASCII.GetString(source,0,4)!="DCX\0"||Encoding.ASCII.GetString(source,40,4)!="KRAK"||Big(source,20)!=76)throw new InvalidDataException("Invalid DCX header");
                int length=Big(source,28),compressed=Big(source,32);
                if(length<=0||length>1024*1024*1024||compressed<=0||compressed>source.Length-76)throw new InvalidDataException("Invalid DCX size");
                long capacity=OodleLZ_GetDecodeBufferSize(length,true);
                Console.WriteLine($"DECODE_PLAN compressed={compressed} expected={length} allocation={capacity}");
                if(capacity<length||capacity>length+1024L*1024)throw new InvalidDataException("Invalid decoder capacity");
                var input=new byte[compressed];Buffer.BlockCopy(source,76,input,0,compressed);
                decoded=new byte[capacity];MemoryReport("allocated");
                long returned=OodleLZ_Decompress(input,input.Length,decoded,length,1,0,2,IntPtr.Zero,0,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,0,3);
                Console.WriteLine($"DECODE_RETURN actual={returned} expected={length}");MemoryReport("after_decode");
                if(returned!=length){Console.WriteLine("RESULT=DECODE_FAILED; return value alone does not prove insufficient memory");return 2;}
                Array.Resize(ref decoded,length);
            }
            Console.WriteLine($"RESULT=SUCCESS decoded_bytes={decoded.Length} decoded_sha256={AssetHash.Of(decoded)}");return 0;
        }catch(Exception e){while(e is TargetInvocationException&&e.InnerException!=null)e=e.InnerException;MemoryReport("exception");Console.Error.WriteLine($"RESULT=EXCEPTION type={e.GetType().FullName} hresult=0x{e.HResult:X8}\n{e}");return 1;}
    }
}
