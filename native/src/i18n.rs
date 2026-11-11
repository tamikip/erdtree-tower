pub fn t(value:impl AsRef<str>)->String {
    let value=value.as_ref();
    if let Some((label,id))=value.split_once("##") {
        format!("{}##{id}",erdtree_tower::localization::text(label))
    } else {erdtree_tower::localization::text(value).into_owned()}
}

pub fn matches(value:&str,query:&str)->bool {
    let query=query.trim().to_lowercase();
    value.to_lowercase().contains(&query)||erdtree_tower::localization::translate_en(value).to_lowercase().contains(&query)
}
