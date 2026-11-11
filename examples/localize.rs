use std::{env,fs};
fn main() {
    let args:Vec<_>=env::args().collect();assert_eq!(args.len(),3,"localize <input-json> <output-json>");
    let input:Vec<String>=serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let translated:Vec<_>=input.iter().map(|text|erdtree_tower::localization::translate_en(text).into_owned()).collect();
    let missing=translated.iter().filter(|s|s.chars().any(|c|('\u{3400}'..='\u{9fff}').contains(&c))).count();
    fs::write(&args[2],serde_json::to_vec_pretty(&translated).unwrap()).unwrap();
    eprintln!("Translated {} entries; {missing} still contain Chinese",input.len());
}
