use ambermap::leveldb::{keys::Dimension, reader::WorldDb};
use std::collections::HashMap;

fn main() {
    let world = std::env::args().nth(1).expect("usage: sign_dump <world>");
    let db = WorldDb::open(&world).unwrap();
    for dim in [Dimension::Overworld, Dimension::Nether, Dimension::TheEnd] {
        let payloads = db.block_entity_payloads(dim);
        let mut ids: HashMap<String, usize> = HashMap::new();
        let mut parse_fail = 0usize;
        for p in &payloads {
            let d = ambermap::leveldb::decompress_payload(p).unwrap_or_else(|_| p.clone());
            match ambermap::nbt::reader::NbtReader::new(&d).read_root_compound() {
                Ok((_, tag)) => {
                    let id = tag
                        .get_string("id")
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "<no id>".to_string());
                    *ids.entry(id).or_default() += 1;
                }
                Err(_) => parse_fail += 1,
            }
        }
        let mut ids: Vec<_> = ids.into_iter().collect();
        ids.sort();
        println!(
            "dim={dim:?} payloads={} parse_fail={}",
            payloads.len(),
            parse_fail
        );
        for (id, n) in ids {
            println!("  {id}: {n}");
        }
    }
}
