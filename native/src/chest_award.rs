use std::{fs,path::Path,io::Write};

#[derive(Debug,PartialEq,Eq)]
pub struct Receipt {pub item:u32,pub target:u64}
impl Receipt {
    pub fn needs_grant(&self,count:u64)->bool {count<self.target}
    fn decode(text:&str)->Result<Self,String> {
        let fields:Vec<_>=text.split_whitespace().collect();
        if fields.len()!=3||fields[0]!="1" {return Err("宝箱领取记录格式无效".into());}
        let item=fields[1].parse::<u32>().map_err(|_|"宝箱物品编号无效")?;
        let target=fields[2].parse::<u64>().map_err(|_|"宝箱数量无效")?;
        if target==0||erdtree_tower::rewards::lookup(item).is_none(){return Err("宝箱领取记录无效".into());}
        Ok(Self{item,target})
    }
    pub fn load(path:&Path)->Result<Option<Self>,String> {
        match fs::read_to_string(path) {
            Ok(text)=>Self::decode(&text).map(Some),
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(None),
            Err(_)=>Err("无法读取宝箱领取记录".into()),
        }
    }
    pub fn begin(path:&Path,item:u32,count:u64)->Result<Self,String> {
        let target=count.checked_add(1).ok_or("宝箱物品数量超限")?;
        let text=format!("1 {item} {target}\n");let receipt=Self::decode(&text)?;
        let mut file=fs::OpenOptions::new().create_new(true).write(true).open(path).map_err(|_|"无法创建宝箱领取记录，未发放物品")?;
        file.write_all(text.as_bytes()).and_then(|_|file.sync_all()).map_err(|_|"无法保存宝箱领取记录，未发放物品")?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_receipt_is_reused_without_reroll_or_duplicate_delivery() {
        let path=std::env::temp_dir().join(format!("tower-chest-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let receipt=Receipt::begin(&path,2000000,2).unwrap();
        assert_eq!(receipt.target,3);assert!(receipt.needs_grant(2));assert!(!receipt.needs_grant(3));
        let retry=Receipt::load(&path).unwrap().unwrap();assert_eq!(retry,receipt);
        assert!(Receipt::begin(&path,3000000,0).is_err());
        assert_eq!(Receipt::load(&path).unwrap().unwrap(),receipt);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn malformed_receipts_cannot_trigger_awards() {
        for value in ["", "1 2000000 0", "1 2000000 3 extra", "2 2000000 3", "1 -1 3"] {
            assert!(Receipt::decode(value).is_err());
        }
    }
}
