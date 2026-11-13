use erdtree_tower::law::{Law,NODES};

#[test]
fn resistance_paths_are_independent_and_preserve_old_codes() {
    assert_eq!(NODES.len(),36);
    let elements=Law::default().toggle(27).toggle(28);
    let status=Law::default().toggle(29).toggle(30);
    assert_eq!(elements.selected(),vec![27,28]);
    assert_eq!(status.selected(),vec![29,30]);
    assert_eq!(elements.toggle(27),Law::default());
    assert_eq!(status.toggle(29),Law::default());
    assert_eq!(Law::default().toggle(30),status,"choosing II also chooses I");
    for mask in [0,(1<<27)-1,elements.0,status.0,(1<<31)-1,u64::from(u32::MAX),(1u64<<34)-1] {
        let law=Law(mask);assert!(law.valid());
        assert_eq!(Law::parse(&law.code()),Some(law.current()),"old codes stay readable; the retired lone wolf is dropped");
    }
    assert_eq!(Law((1<<27)-1).code().len(),7);
    assert_eq!(status.code().len(),8);
    assert_eq!(Law::default().toggle(31),Law(1<<31),"empowered affixes are independent");
    let empowered=Law::default().toggle(3).toggle(4).toggle(5).toggle(31);
    assert!(empowered.empowered_affixes()&&empowered.valid());
    assert_eq!(Law::parse(&empowered.code()),Some(empowered));
    assert_eq!(empowered.toggle(5),Law::default().toggle(3).toggle(4).toggle(31));
    assert!(Law(1<<31).valid());
}

#[test]
fn economy_penalties_are_reduced() {
    let one=Law::default().toggle(19);
    let two=one.toggle(20);
    assert_eq!((one.runes(1000),two.runes(1000)),(850,700));
    assert_eq!((two.toggle(21).price_percent(),two.toggle(21).toggle(22).price_percent()),(120,140));
    assert_eq!(two.runes(u32::MAX),(u64::from(u32::MAX)*70/100) as u32);
}
