#[derive(Clone, Copy)]
pub struct Arena {
    pub block: i32,
    pub entry: u32,
    pub stone: u32,
    pub name: &'static str,
    pub center:[f32;3],
    pub yaw:f32,
    arrival:Option<[f32;3]>,
}
const LEGACY: [Arena; 3] = [
    Arena { block: 0x2d020000, entry: 45022600, stone: 45020100, name: "宁姆格福竞技场",center:[0.0;3],yaw:0.0,arrival:None },
    Arena { block: 0x2d010000, entry: 45012600, stone: 45010100, name: "盖利德竞技场",center:[0.0;3],yaw:0.0,arrival:None },
    Arena { block: 0x2d000000, entry: 45002600, stone: 45000100, name: "王城竞技场",center:[0.0;3],yaw:0.0,arrival:None },
];
static ARENAS:std::sync::LazyLock<Vec<Arena>>=std::sync::LazyLock::new(||include_str!("../../scene-assets/battlefields.txt").lines().map(|line| {
    let p:Vec<_>=line.split('|').collect();let map:Vec<u8>=p[1][1..].split('_').map(|v|v.parse().unwrap()).collect();
    let basis=p[2].parse::<u32>().unwrap();
    Arena{block:i32::from_be_bytes([map[0],map[1],map[2],map[3]]),entry:basis+2600,stone:basis+100,name:p[3],center:[p[4].parse().unwrap(),p[5].parse().unwrap(),p[6].parse().unwrap()],yaw:p[7].parse::<f32>().unwrap().to_radians(),arrival:Some(std::array::from_fn(|i|p[8+i].parse().unwrap()))}
}).collect());
impl Arena {
    pub fn combat_entry(self)->u32 {self.entry+351}
    pub fn attendant_position(self)->[f32;3] {self.arrival.unwrap_or_else(||self.position([28.35,0.35,-4.06]))}
    pub fn position(self,local:[f32;3])->[f32;3] {
        let (sin,cos)=self.yaw.sin_cos();
        [self.center[0]+local[0]*cos+local[2]*sin,self.center[1]+local[1],self.center[2]-local[0]*sin+local[2]*cos]
    }
    pub fn boss_entity(self,slot:usize)->i32 {self.stone as i32-100+850+slot as i32}
}
pub fn for_act(act: u8) -> Arena {
    LEGACY[usize::from(act.clamp(1,3)-1)]
}
pub fn for_block(block: i32) -> Option<Arena> { ARENAS.iter().chain(LEGACY.iter()).copied().find(|a| a.block == block) }

#[cfg(test)]
mod tests {
    #[test]
    fn combat_arrival_uses_exterior_gate_region_without_moving_services() {
        for act in 1..=3 {
            let arena=super::for_act(act);
            assert_eq!(arena.combat_entry(),arena.entry+351);
            assert_eq!(arena.attendant_position(),[28.35,0.35,-4.06]);
        }
    }
    #[test]
    fn attendants_use_verified_entry_height_instead_of_flat_arena_offsets() {
        for (index,line) in include_str!("../../scene-assets/battlefields.txt").lines().enumerate() {
            let fields:Vec<_>=line.split('|').collect();
            let expected:[f32;3]=std::array::from_fn(|i|fields[8+i].parse().unwrap());
            let arena=super::ARENAS[index];
            assert_eq!(arena.attendant_position(),expected);
            assert_ne!(arena.attendant_position(),arena.position([-16.0,0.1,10.0]));
        }
        let legacy=super::for_block(0x2d020000).unwrap();
        assert_eq!(legacy.attendant_position(),[28.35,0.35,-4.06]);
    }
    #[test]
    fn acts_have_distinct_matching_arena_resources() {
        let arenas = [super::for_act(1),super::for_act(2),super::for_act(3)];
        assert_eq!(arenas[0].block,0x2d020000);
        assert_eq!(arenas[0].entry,45022600);
        assert_eq!(arenas[0].attendant_position(),[28.35,0.35,-4.06]);
        for (i, a) in arenas.iter().enumerate() {
            assert_eq!(super::for_act(i as u8 + 1).block, a.block);
            assert_eq!(super::for_block(a.block).unwrap().stone, a.stone);
            assert_eq!(a.entry - a.stone, 2500);
            assert_eq!(a.position([0.0;3]),a.center);
            assert_eq!(a.boss_entity(0),a.entry as i32-1750);
            assert_eq!(a.block,[0x2d020000,0x2d010000,0x2d000000][i]);
            let front=a.position([0.0,0.0,10.0]);let back=a.position([0.0,0.0,-10.0]);
            assert!((front[0]+back[0]-2.0*a.center[0]).abs()<0.001);
            assert!((front[2]+back[2]-2.0*a.center[2]).abs()<0.001);
            assert!(arenas[..i].iter().all(|b| b.block != a.block));
        }
        assert!(super::for_block(0x0b0a0000).is_none());
    }
}
