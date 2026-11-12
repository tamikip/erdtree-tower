use std::{collections::{BTreeMap,HashMap},sync::Mutex,time::{Duration,Instant}};
use eldenring::cs::{SummonBuddyManager,WorldChrMan};

#[derive(Clone)]
struct Member {key:u64,param:i32,alive:bool}
pub(super) struct Use {goods:u32,before:Vec<u64>,params:Vec<i32>,members:HashMap<u64,(bool,Instant)>}
#[derive(Default)]
struct Ledger {inventory:Vec<(u32,u32)>,observed_inventory:Option<Vec<(u32,u32)>>,uses:Vec<Use>,closed:bool}
#[derive(Debug)]
pub(crate) struct Refund {pub goods:u32,pub quantity:u32,post_spend_max:u32,target:Option<u32>,confirmed:bool}
fn count(inventory:&[(u32,u32)],goods:u32)->u32 {inventory.iter().filter(|(id,_)|*id==goods).fold(0,|sum,(_,n)|sum.saturating_add(*n))}
impl Refund {
    fn grant(&self,current:u32)->Option<u32>{(!self.confirmed&&self.target.is_none()&&current<=self.post_spend_max).then_some(self.quantity)}
    fn requested(&mut self,current:u32){self.target=Some(current.saturating_add(self.quantity));}
    fn confirm(&mut self,current:u32){if self.target.is_some_and(|target|current>=target){self.confirmed=true;}}
}
impl Ledger {
    fn refresh_inventory(&mut self,current:&[(u32,u32)]) {
        if self.closed{return;}
        let previous=self.observed_inventory.as_deref().unwrap_or(&self.inventory);
        let goods:std::collections::BTreeSet<_>=current.iter().map(|(goods,_)|*goods).collect();
        let additions:Vec<_>=goods.into_iter().map(|goods|(goods,count(current,goods).saturating_sub(count(previous,goods)))).filter(|(_,n)|*n>0).collect();
        self.observed_inventory=Some(current.to_vec());
        for (goods,quantity) in additions {
            let before=count(&self.inventory,goods);
            self.inventory.retain(|(id,_)|*id!=goods);
            self.inventory.push((goods,before.saturating_add(quantity)));
        }
    }
    fn accept(&mut self,usage:Use) {
        let used=self.uses.iter().filter(|u|u.goods==usage.goods).count() as u32;
        if !self.closed&&count(&self.inventory,usage.goods)>used{self.uses.push(usage);}
    }
    fn observe(&mut self,members:&[Member],cleanup:bool,now:Instant) {
        if self.closed{return;}
        for usage in &mut self.uses {
            for (key,(alive,seen)) in &mut usage.members {
                if let Some(member)=members.iter().find(|m|m.key==*key) {
                    *alive=member.alive&&(usage.params.is_empty()||usage.params.contains(&member.param));*seen=now;
                } else if !cleanup||now.duration_since(*seen)>Duration::from_secs(2) {*alive=false;}
            }
        }
        for member in members {
            if let Some(usage)=self.uses.iter_mut().rev().find(|u|!u.before.contains(&member.key)&&(u.params.is_empty()||u.params.contains(&member.param))) {
                usage.members.insert(member.key,(member.alive,now));
            }
        }
    }
    fn settle(&mut self)->Vec<Refund> {
        if self.closed{return Vec::new();}self.closed=true;
        let mut counts=BTreeMap::<u32,(u32,u32)>::new();
        for usage in &self.uses {let entry=counts.entry(usage.goods).or_default();entry.0+=1;entry.1+=u32::from(usage.members.values().any(|(alive,_)|*alive));}
        counts.into_iter().filter_map(|(goods,(spent,surviving))| {
            let before=count(&self.inventory,goods);
            (surviving>0).then_some(Refund{goods,quantity:surviving,post_spend_max:before.saturating_sub(spent),target:None,confirmed:false})
        }).collect()
    }
}

static LEDGER:Mutex<Ledger>=Mutex::new(Ledger{inventory:Vec::new(),observed_inventory:None,uses:Vec::new(),closed:true});
fn local_members(manager:&SummonBuddyManager,owner:i32)->Vec<Member> {
    manager.groups.iter().filter(|pair|pair.first==owner).flat_map(|pair|pair.second.iter()).filter(|group|!group.is_remote).map(|group| {
        let chr=unsafe {group.chr_ins.as_ref()};let handle=chr.field_ins_handle;
        Member{key:(u64::from(handle.block_id.0 as u32)<<32)|u64::from(handle.selector.0),param:group.buddy_param_id,
            alive:chr.modules.data.hp>0&&!chr.chr_flags1c5.death_flag()&&(!group.disappear_requested||manager.disappear_requested)}
    }).collect()
}
pub(super) fn capture(manager:&SummonBuddyManager,goods:i32,effect:i32)->Option<Use> {
    let goods=(goods>0).then_some(goods as u32&0x0fffffff)?;
    let player=unsafe {eldenring::cs::PlayerIns::local_player()}.ok()?;
    let owner=unsafe {player.player_game_data.as_ref()}.character_event_id as i32;
    let before=local_members(manager,owner).into_iter().map(|m|m.key).collect();
    let params=manager.trigger_speffect_to_buddy_map.iter().filter_map(|(trigger,param)|(*trigger==effect).then_some(*param)).collect();
    Some(Use{goods,before,params,members:HashMap::new()})
}
pub(super) fn accepted(usage:Use) {
    if let Ok(mut ledger)=LEDGER.lock(){let count=ledger.uses.len();let goods=usage.goods;let params=usage.params.clone();ledger.accept(usage);
        if ledger.uses.len()!=count {super::super::log(format!("ASH_GROUP accepted use={} goods={goods} buddy_params={params:?}",count+1));}
        else {super::super::log(format!("ASH_GROUP ignored request goods={goods}: encounter closed, limit reached, or absent from starting inventory"));}}
}
pub(crate) fn begin(before:&[(u32,u32)]) {if let Ok(mut ledger)=LEDGER.lock(){*ledger=Ledger{inventory:before.to_vec(),..Default::default()};}}
pub(crate) fn tracking()->bool {LEDGER.lock().is_ok_and(|ledger|!ledger.closed)}
pub(crate) fn close(){if let Ok(mut ledger)=LEDGER.lock(){ledger.closed=true;}}
pub(crate) fn observe(world:&WorldChrMan) {
    let Some(player)=world.main_player.as_ref() else{return;};
    let manager=&world.summon_buddy_manager;
    let owner=unsafe {player.player_game_data.as_ref()}.character_event_id as i32;
    let members=local_members(manager,owner);
    let inventory=super::super::loadout::ash_snapshot(unsafe {player.player_game_data.as_ref()}).ok();
    if let Ok(mut ledger)=LEDGER.lock() {
        if let Some(inventory)=inventory {ledger.refresh_inventory(&inventory);}
        if !ledger.closed&&ledger.uses.is_empty()&&!members.is_empty()&&manager.active_summon_goods_id>0&&!manager.disappear_requested&&super::ACTIVE.load(std::sync::atomic::Ordering::Acquire) {
            // 召唤请求没被追踪到时（没装上追踪，或开战前就召唤了：倒数、敌人还没出现时追踪还没开），按场上的召唤物补记这一次，
            // 否则胜利时召唤物还活着也不返还骨灰（玩家日志：每场都是开战前召唤，ASH_GROUP victory refunds=[]）
            let params=manager.trigger_speffect_to_buddy_map.iter().filter_map(|(trigger,param)|(*trigger==manager.active_summon_speffect_id as i32).then_some(*param)).collect();
            let goods=manager.active_summon_goods_id as u32&0x0fffffff;
            ledger.accept(Use{goods,before:Vec::new(),params,members:HashMap::new()});
            if !ledger.uses.is_empty() {super::super::log(format!("ASH_GROUP untracked summon recorded goods={goods} members={}",members.len()));}
        }
        ledger.observe(&members,manager.disappear_requested,Instant::now());
    }
}
pub(crate) fn settle(world:&WorldChrMan,summoner:bool)->Vec<Refund> {
    observe(world);
    let Ok(mut ledger)=LEDGER.lock() else{return Vec::new();};
    if !summoner {ledger.closed=true;return Vec::new();}
    if !ledger.closed {for (index,usage) in ledger.uses.iter().enumerate() {
        super::super::log(format!("ASH_GROUP victory use={} goods={} living={} tracked={}",index+1,usage.goods,usage.members.values().filter(|(alive,_)|*alive).count(),usage.members.len()));
    }}
    let plan=ledger.settle();
    super::super::log(format!("ASH_GROUP victory refunds={:?}",plan.iter().map(|r|(r.goods,r.quantity)).collect::<Vec<_>>()));
    plan
}
pub(crate) fn pump(data:&mut eldenring::cs::PlayerGameData,plan:&mut [Refund])->Result<bool,String> {
    let snapshot=super::super::loadout::ash_snapshot(data)?;
    let mut grants=Vec::new();
    for (index,refund) in plan.iter_mut().enumerate() {
        let current=count(&snapshot,refund.goods);
        refund.confirm(current);
        if let Some(quantity)=refund.grant(current) {grants.push((index,current,quantity));}
    }
    if !grants.is_empty() {
        let items:Vec<_>=grants.iter().map(|&(index,_,quantity)|((0x40000000|plan[index].goods) as i32,quantity as i32,-1)).collect();
        super::super::items::give(&items)?;
        for (index,current,quantity) in grants {let refund=&mut plan[index];refund.requested(current);
            super::super::log(format!("ASH_GROUP refund requested goods={} quantity={quantity} target={:?}",refund.goods,refund.target));}
    }
    Ok(plan.iter().all(|r|r.confirmed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shop_purchase_is_tracked_and_portal_cannot_refund_it_twice() {
        let mut ledger=Ledger::default();
        ledger.refresh_inventory(&[(212000,1)]);
        ledger.accept(usage(212000,&[],212000));
        ledger.refresh_inventory(&[(212000,1)]);
        ledger.accept(usage(212000,&[],212000));
        assert_eq!(ledger.uses.len(),1,"pending consumption is not a new purchase");
        ledger.refresh_inventory(&[]);
        ledger.observe(&[member(1,212000,true),member(2,212000,true)],false,Instant::now());
        let refunds=ledger.settle();
        assert_eq!((refunds[0].goods,refunds[0].quantity),(212000,1));
        assert!(ledger.settle().is_empty(),"victory and portal share one receipt");
    }
    #[test]
    fn event_precombat_use_survives_empty_post_summon_inventory() {
        let mut ledger=Ledger{inventory:vec![(212000,1)],..Default::default()};
        ledger.accept(usage(212000,&[],212000));
        ledger.observe(&[member(1,212000,true),member(2,212000,true)],false,Instant::now());
        let refunds=ledger.settle();
        assert_eq!(refunds.len(),1);
        assert_eq!((refunds[0].goods,refunds[0].quantity),(212000,1),"one ash, not two skeletons");
        assert_eq!(refunds[0].grant(0),Some(1));
        assert!(ledger.settle().is_empty(),"settlement cannot pay twice");
    }
    #[test]
    fn summoner_resummons_after_two_dead_groups_still_refund_the_survivor() {
        let mut ledger=Ledger{inventory:vec![(270039,3)],..Default::default()};
        for index in 0..3 {
            let before:Vec<_>=(1..=index).collect();
            ledger.accept(usage(270039,&before,1600039000));
            ledger.observe(&[member(index+1,1600039000,index==2)],false,Instant::now());
        }
        assert_eq!(ledger.uses.len(),3);
        ledger.accept(usage(270039,&[1,2,3],1600039000));
        assert_eq!(ledger.uses.len(),3,"cannot refund more uses than the held inventory");
        let plan=ledger.settle();
        assert_eq!(plan.iter().map(|r|(r.goods,r.quantity)).collect::<Vec<_>>(),[(270039,1)]);
        assert!(ledger.settle().is_empty());
    }
    #[test]
    fn custom_spirits_refund_each_surviving_group_at_its_original_level() {
        let mut ledger=Ledger{inventory:vec![(270039,1),(272011,1)],..Default::default()};
        ledger.accept(usage(270039,&[],1600039000));
        ledger.accept(usage(272011,&[1],1600091000));
        ledger.observe(&[member(1,1600039000,true),member(2,1600091000,false)],false,Instant::now());
        let refunds=ledger.settle();
        assert_eq!(refunds.iter().map(|r|(r.goods,r.quantity)).collect::<Vec<_>>(),[(270039,1)]);
    }
    fn usage(goods:u32,before:&[u64],param:i32)->Use {Use{goods,before:before.to_vec(),params:vec![param],members:HashMap::new()}}
    fn member(key:u64,param:i32,alive:bool)->Member {Member{key,param,alive}}
    fn two()->Ledger {let mut l=Ledger{inventory:vec![(100,1),(200,1)],..Default::default()};l.accept(usage(100,&[],10));l.accept(usage(200,&[1,2],20));l}
    #[test]fn a_summon_made_before_tracking_started_is_still_refunded_once(){let mut l=Ledger{inventory:vec![(100,1)],..Default::default()};l.accept(usage(100,&[],10));l.observe(&[member(1,10,true)],false,Instant::now());assert!(l.uses.len()==1);l.accept(usage(100,&[],10));assert_eq!(l.uses.len(),1,"only one ash was held");let p=l.settle();assert_eq!(p.iter().map(|p|(p.goods,p.quantity)).collect::<Vec<_>>(),[(100,1)]);}
    #[test]fn each_use_is_refunded_once_even_when_it_spawns_several_members(){let mut l=two();l.observe(&[member(1,10,false),member(2,10,true),member(3,20,true)],false,Instant::now());let p=l.settle();assert_eq!(p.iter().map(|p|(p.goods,p.quantity)).collect::<Vec<_>>(),[(100,1),(200,1)]);assert!(l.settle().is_empty());}
    #[test]fn one_dead_group_does_not_borrow_the_other_groups_survival(){for first_alive in [true,false]{let mut l=two();l.observe(&[member(1,10,first_alive),member(2,10,first_alive),member(3,20,!first_alive)],false,Instant::now());let p=l.settle();assert_eq!(p.len(),1);assert_eq!(p[0].goods,if first_alive{100}else{200});}}
    #[test]fn same_ash_used_twice_counts_surviving_uses_not_creatures(){let mut l=Ledger{inventory:vec![(100,2)],..Default::default()};l.accept(usage(100,&[],10));l.accept(usage(100,&[1],10));l.observe(&[member(1,10,true),member(2,10,true)],false,Instant::now());let p=l.settle();assert_eq!(p.len(),1);assert_eq!((p[0].quantity,p[0].post_spend_max),(2,0));}
    #[test]fn same_ash_with_only_second_alive_refunds_one(){let mut l=Ledger{inventory:vec![(100,2)],..Default::default()};l.accept(usage(100,&[],10));l.accept(usage(100,&[1],10));l.observe(&[member(1,10,false),member(2,10,true)],false,Instant::now());let p=l.settle();assert_eq!((p[0].quantity,p[0].post_spend_max),(1,0));}
    #[test]fn cleanup_grace_never_resurrects_an_observed_dead_group(){let start=Instant::now();let mut l=two();l.observe(&[member(1,10,true),member(3,20,true)],false,start);l.observe(&[member(1,10,false),member(3,20,true)],false,start+Duration::from_millis(10));l.observe(&[],true,start+Duration::from_millis(20));let p=l.settle();assert_eq!(p.len(),1);assert_eq!(p[0].goods,200);}
    #[test]fn vanished_group_without_cleanup_and_unrelated_members_are_not_refunded(){let mut l=two();let start=Instant::now();l.observe(&[member(1,10,true)],false,start);l.observe(&[member(99,99,true)],false,start+Duration::from_millis(10));assert!(l.settle().is_empty());}
    #[test]fn delayed_first_group_members_keep_their_buddy_param_identity(){let mut l=two();l.observe(&[member(1,10,false),member(2,10,false),member(3,20,false),member(4,10,true)],false,Instant::now());let p=l.settle();assert_eq!(p.len(),1);assert_eq!(p[0].goods,100);}
    #[test]fn async_refund_waits_for_both_consumptions_and_never_grants_twice(){let mut r=Refund{goods:100,quantity:2,post_spend_max:0,target:None,confirmed:false};assert_eq!(r.grant(1),None);assert_eq!(r.grant(0),Some(2));r.requested(0);assert_eq!(r.grant(0),None);r.confirm(1);assert!(!r.confirmed);r.confirm(2);assert!(r.confirmed);assert_eq!(r.grant(0),None);}
    #[test]fn separate_inventory_entries_are_counted_and_unknown_goods_cannot_be_refunded(){let mut l=Ledger{inventory:vec![(100,1),(100,1)],..Default::default()};l.accept(usage(999,&[],10));assert!(l.uses.is_empty());l.accept(usage(100,&[],10));l.accept(usage(100,&[1],10));l.accept(usage(100,&[1,2],10));assert_eq!(l.uses.len(),2);l.observe(&[member(1,10,true),member(2,10,true)],false,Instant::now());assert_eq!(l.settle()[0].quantity,2);assert_eq!(count(&[(100,1),(100,1)],100),2);}
}
