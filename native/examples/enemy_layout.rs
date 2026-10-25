use eldenring::cs::*;
fn main() {
    println!(
        "creator={} last={} debug={} sets={} model={} npc={} handle={} entry={} modules={}",
        std::mem::offset_of!(WorldChrMan, debug_chr_creator),
        std::mem::offset_of!(CSDebugChrCreator, last_created_chr),
        std::mem::offset_of!(WorldChrMan, debug_chr_set),
        std::mem::offset_of!(WorldChrMan, chr_sets),
        std::mem::offset_of!(ChrIns, npc_id),
        std::mem::offset_of!(ChrIns, npc_param_id),
        std::mem::offset_of!(ChrIns, field_ins_handle),
        std::mem::offset_of!(ChrIns, chr_set_entry),
        std::mem::offset_of!(ChrIns, modules)
    );
}
