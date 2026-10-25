use erdtree_tower::{Class, Run};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run = match args.as_slice() {
        [] => Run::new(42, Class::Samurai),
        [flag, seed] if flag == "--seed" => {
            Run::new(seed.parse().map_err(|_| "无效随机种子")?, Class::Samurai)
        }
        [flag, path] if flag == "--validate-save" => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let r = Run::decode(&text)?;
            println!("存档校验通过，当前状态：{:?}", r.phase);
            return Ok(());
        }
        _ => return Err("用法：erdtree-tower [--seed 数字 | --validate-save 文件]".into()),
    };
    println!("黄金树爬塔 — 规则检查工具（不运行游戏战斗）");
    println!(
        "种子：{}｜圣杯瓶：{}/{}｜武器等级：{}",
        run.seed, run.flasks, run.flask_limit, run.weapon_level
    );
    for layer in 0..run.floors() {
        let nodes: Vec<_> = run
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.layer == layer)
            .map(|(id, n)| format!("{id}:{} → {:?}", n.kind.label(), n.next))
            .collect();
        println!("第 {:02} 层｜{}", layer + 1, nodes.join("  |  "));
    }
    Ok(())
}
