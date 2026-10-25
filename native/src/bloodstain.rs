use std::sync::OnceLock;

pub fn clear()->Result<(),String> {
    static ADDRESS:OnceLock<Result<usize,String>>=OnceLock::new();
    // Native save-data reset also used when starting NG+: clears the stain, not held runes.
    let address=*ADDRESS.get_or_init(||super::items::checked_function(0x255900,&[
        0x48,0x8b,0x05,0xb1,0xf6,0xb0,0x03,0x48,0x8b,0x48,0x48,0x48,0x85,0xc9,0x74,0x6f,
    ])).as_ref().map_err(Clone::clone)?;
    let reset:unsafe extern "C" fn()=unsafe {std::mem::transmute(address)};
    unsafe {reset();}
    super::log("Previous-run bloodstain cleared before hub warp");
    Ok(())
}
