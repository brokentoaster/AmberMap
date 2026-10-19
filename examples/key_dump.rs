use ambermap::leveldb::reader::WorldDb;
use ambermap::leveldb::Dimension;
use ambermap::markers::{collect_markers, parse_sign_payload};

fn main() {
    let world = std::env::args().nth(1).unwrap();
    let db = WorldDb::open(&world).unwrap();
    let m = collect_markers(&db, Dimension::Overworld);
    println!("overworld signs: {}", m.signs.len());
    let hit: Vec<_> = m
        .signs
        .iter()
        .filter(|s| s.x == 93 && s.z == -272)
        .collect();
    println!("target sign found: {hit:?}");

    // Also check raw payload parsing for the chunk containing the sign
    let cx = 93 / 16;
    let cz = (-272i32) / 16;
    let mut count = 0;
    let mut dup = 0;
    for payload in db.block_entity_payloads(Dimension::Overworld) {
        let d = ambermap::leveldb::decompress_payload(&payload).unwrap_or(payload);
        match parse_sign_payload(&d) {
            Some(s) => {
                if s.x == 93 && s.z == -272 {
                    println!("parse_sign_payload hit: {s:?}");
                }
                count += 1;
            }
            None => dup += 1,
        }
    }
    let _ = (cx, cz);
    println!("parsed signs via parse_sign_payload: {count}, unparsed BE records: {dup}");
}
