// 界面音效：scene-assets/sfx/*.wav（Kenney 的 CC0 现成音效，scene-builder/ui_sfx.py 从 art-source/sfx-kenney/ 转换），用 Windows 自带的 winmm（waveOut）从内存播放。
// 每次播放单独开一个输出句柄，由系统混音，几个音效可以叠在一起；播完由后台线程关掉。音量按 tower-options.ini 的 sfx（0–100）在载入时缩放。
use std::{collections::HashMap,sync::{LazyLock,atomic::{AtomicUsize,Ordering}},time::{Duration,Instant}};

#[repr(C)] struct WaveFormat {tag:u16,channels:u16,rate:u32,bytes_per_second:u32,block_align:u16,bits:u16,extra:u16}
#[repr(C)] struct WaveHeader {data:*mut u8,length:u32,recorded:u32,user:usize,flags:u32,loops:u32,next:*mut WaveHeader,reserved:usize}
#[link(name="winmm",kind="raw-dylib")]
unsafe extern "system" {
    fn waveOutOpen(handle:*mut isize,device:u32,format:*const WaveFormat,callback:usize,instance:usize,flags:u32)->u32;
    fn waveOutPrepareHeader(handle:isize,header:*mut WaveHeader,size:u32)->u32;
    fn waveOutWrite(handle:isize,header:*mut WaveHeader,size:u32)->u32;
    fn waveOutUnprepareHeader(handle:isize,header:*mut WaveHeader,size:u32)->u32;
    fn waveOutReset(handle:isize)->u32;
    fn waveOutClose(handle:isize)->u32;
}
const WAVE_MAPPER:u32=u32::MAX;const WHDR_DONE:u32=1;
/// 同时播放的上限（超过的直接丢掉，免得连点时开太多句柄）
const MAX_VOICES:usize=8;
static VOICES:AtomicUsize=AtomicUsize::new(0);

struct Sound {channels:u16,rate:u32,samples:Vec<i16>}
/// 只认 16 位 PCM：找 fmt 与 data 两个块
fn parse(bytes:&[u8])->Option<Sound> {
    if bytes.len()<12||&bytes[0..4]!=b"RIFF"||&bytes[8..12]!=b"WAVE" {return None;}
    let (mut at,mut format,mut data)=(12,None,None);
    while at+8<=bytes.len() {
        let id=&bytes[at..at+4];let len=u32::from_le_bytes(bytes[at+4..at+8].try_into().ok()?) as usize;
        let body=bytes.get(at+8..at+8+len)?;
        if id==b"fmt " && body.len()>=16 {format=Some((u16::from_le_bytes([body[0],body[1]]),u16::from_le_bytes([body[2],body[3]]),u32::from_le_bytes(body[4..8].try_into().ok()?),u16::from_le_bytes([body[14],body[15]])));}
        if id==b"data" {data=Some(body);}
        at+=8+len+(len&1);
    }
    let ((tag,channels,rate,bits),data)=(format?,data?);
    (tag==1&&bits==16&&channels>0).then(||Sound{channels,rate,samples:data.chunks_exact(2).map(|c|i16::from_le_bytes([c[0],c[1]])).collect()})
}
const NAMES:[&str;17]=["open","tick1","tick2","tick3","stamp","record","hover","confirm","lock1","lock2","lock3","lock4","lock5","lock6","sweep","total","close"];
static SOUNDS:LazyLock<HashMap<&'static str,Sound>>=LazyLock::new(|| {
    let volume=f32::from(crate::options::get().sfx)/100.0;
    let dir=crate::LOG_PATH.with_file_name("scene-assets").join("sfx");
    let mut sounds=HashMap::new();
    for name in NAMES {
        match std::fs::read(dir.join(format!("{name}.wav"))).ok().and_then(|b|parse(&b)) {
            Some(mut sound)=>{for s in &mut sound.samples {*s=(f32::from(*s)*volume) as i16;}sounds.insert(name,sound);},
            None=>crate::log(format!("SFX missing or unsupported: {name}.wav")),
        }
    }
    sounds
});
/// 播放一个音效（不阻塞；音量为 0、文件缺失或同时播放太多时什么都不做）
pub fn play(name:&'static str) {
    if crate::options::get().sfx==0 {return;}
    let Some(sound)=SOUNDS.get(name) else{return};
    if VOICES.fetch_add(1,Ordering::AcqRel)>=MAX_VOICES {VOICES.fetch_sub(1,Ordering::AcqRel);return;}
    let (channels,rate)=(sound.channels,sound.rate);let mut bytes:Vec<u8>=sound.samples.iter().flat_map(|s|s.to_le_bytes()).collect();
    std::thread::spawn(move || {
        let format=WaveFormat{tag:1,channels,rate,bytes_per_second:rate*u32::from(channels)*2,block_align:channels*2,bits:16,extra:0};
        let mut handle:isize=0;
        if unsafe {waveOutOpen(&mut handle,WAVE_MAPPER,&format,0,0,0)}==0 {
            let mut header=WaveHeader{data:bytes.as_mut_ptr(),length:bytes.len() as u32,recorded:0,user:0,flags:0,loops:0,next:std::ptr::null_mut(),reserved:0};
            let size=std::mem::size_of::<WaveHeader>() as u32;
            if unsafe {waveOutPrepareHeader(handle,&mut header,size)}==0 {
                if unsafe {waveOutWrite(handle,&mut header,size)}==0 {
                    let limit=Instant::now()+Duration::from_secs_f32(bytes.len() as f32/format.bytes_per_second as f32+0.5);
                    while unsafe {std::ptr::read_volatile(&header.flags)}&WHDR_DONE==0&&Instant::now()<limit {std::thread::sleep(Duration::from_millis(15));}
                }
                unsafe {waveOutReset(handle);waveOutUnprepareHeader(handle,&mut header,size);}
            }
            unsafe {waveOutClose(handle);}
        }
        VOICES.fetch_sub(1,Ordering::AcqRel);
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_16_bit_pcm_and_rejects_other_files() {
        let mut wav=b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        wav.extend(16u32.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(1u16.to_le_bytes());wav.extend(44100u32.to_le_bytes());wav.extend(88200u32.to_le_bytes());wav.extend(2u16.to_le_bytes());wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");wav.extend(4u32.to_le_bytes());wav.extend(100i16.to_le_bytes());wav.extend((-5i16).to_le_bytes());
        let sound=super::parse(&wav).unwrap();assert_eq!((sound.channels,sound.rate,sound.samples),(1,44100,vec![100,-5]));
        assert!(super::parse(b"not a wav").is_none());
    }
}
