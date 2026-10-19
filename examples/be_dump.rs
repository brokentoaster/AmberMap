use ambermap::leveldb::sstable::read_sstable;
use ambermap::leveldb::decompress_payload;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn main() {
    let world = std::env::args().nth(1).unwrap();
    let db_dir = {
        let p = PathBuf::from(&world);
        if p.join("db").is_dir() {
            p.join("db")
        } else {
            p
        }
    };
    let mut entries: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    for entry in fs::read_dir(&db_dir).unwrap().flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e == "ldb" || e == "sst")
        {
            let _ = read_sstable(&path, &mut entries);
        }
    }
    // search all BE records (tag 0x31, 9-byte key = overworld) within +-8 chunks
    let (tx, tz) = (93i32.div_euclid(16), (-273i32).div_euclid(16));
    for (k, v) in &entries {
        if k.len() == 9 && k[8] == 0x31 {
            let x = i32::from_le_bytes([k[0], k[1], k[2], k[3]]);
            let z = i32::from_le_bytes([k[4], k[5], k[6], k[7]]);
            if x.abs_diff(tx) <= 8 && z.abs_diff(tz) <= 8 {
                let d = decompress_payload(v).unwrap_or_else(|_| v.clone());
                let mut r = ambermap::nbt::reader::NbtReader::new(&d);
                if let Ok((_, tag)) = r.read_root_compound() {
                    let id = tag.get_string("id").unwrap_or("");
                    let bx = tag.get_i32("x").unwrap_or(0);
                    let by = tag.get_i32("y").unwrap_or(0);
                    let bz = tag.get_i32("z").unwrap_or(0);
                    println!("({bx}, {by}, {bz}) id={id}");
                    if id.to_lowercase().contains("sign") {
                        println!("   full: {tag:?}");
                    }
                } else {
                    println!("chunk ({x},{z}) BE parse fail, len={}", d.len());
                }
            }
        }
    }
}
