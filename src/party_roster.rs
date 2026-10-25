use crate::RuleResult;
pub const MAX_PLAYERS:usize=8;

#[derive(Clone,Copy)]
pub struct Member {pub id:u64,pub local:bool,pub host:bool,pub joining:bool}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Roster {pub local:u64,pub host:u64,pub members:Vec<u64>}
impl Roster {
    pub fn read(host:u64,local_is_host:bool,entries:&[Member])->RuleResult<Self> {
        if host==0||entries.is_empty()||entries.len()>MAX_PLAYERS {return Err("会话名单未就绪".into());}
        let mut local=None;let mut members=Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.id==0||entry.joining||members.contains(&entry.id) {return Err("会话成员不完整或重复".into());}
            if entry.host!=(entry.id==host) {return Err("会话房主标记不一致".into());}
            if entry.local&&local.replace(entry.id).is_some() {return Err("会话存在多个本机成员".into());}
            members.push(entry.id);
        }
        let local=local.ok_or("会话缺少本机成员")?;
        if !members.contains(&host)||(local==host)!=local_is_host {return Err("会话房主身份不一致".into());}
        members.sort_unstable();
        Ok(Self{local,host,members})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn member(id:u64,local:bool)->Member {Member{id,local,host:id==10,joining:false}}
    #[test]
    fn every_member_agrees_for_two_to_eight_and_ninth_is_rejected() {
        for count in 2..=MAX_PLAYERS {
            for local in 1..=count {
                let entries=(1..=count).rev().map(|id|Member{id:id as u64,local:id==local,host:id==1,joining:false}).collect::<Vec<_>>();
                let roster=Roster::read(1,local==1,&entries).unwrap();
                assert_eq!(roster.local,local as u64);
                assert_eq!(roster.members,(1..=count as u64).collect::<Vec<_>>());
            }
        }
        let entries=(1..=MAX_PLAYERS+1).map(|id|Member{id:id as u64,local:id==1,host:id==1,joining:false}).collect::<Vec<_>>();
        assert!(Roster::read(1,true,&entries).is_err());
    }
    #[test]
    fn host_and_guest_agree_on_sorted_members_and_host() {
        let host=Roster::read(10,true,&[member(20,false),member(10,true)]).unwrap();
        let guest=Roster::read(10,false,&[member(10,false),member(20,true)]).unwrap();
        assert_eq!(host.members,guest.members);assert_eq!(host.host,guest.host);
        assert_eq!((host.local,guest.local),(10,20));
    }
    #[test]
    fn partial_duplicate_or_conflicting_identity_never_becomes_a_team() {
        assert!(Roster::read(10,true,&[]).is_err());
        assert!(Roster::read(10,true,&[member(10,false)]).is_err());
        assert!(Roster::read(10,true,&[member(10,true),member(20,true)]).is_err());
        assert!(Roster::read(10,true,&[member(10,true),member(10,false)]).is_err());
        assert!(Roster::read(10,false,&[member(10,true)]).is_err());
        assert!(Roster::read(30,false,&[member(10,false),member(20,true)]).is_err());
        let mut joining=member(20,false);joining.joining=true;
        assert!(Roster::read(10,true,&[member(10,true),joining]).is_err());
        let mut conflict=member(20,false);conflict.host=true;
        assert!(Roster::read(10,true,&[member(10,true),conflict]).is_err());
    }
}
