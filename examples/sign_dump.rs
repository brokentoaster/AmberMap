use ambermap::leveldb::{keys::Dimension, reader::WorldDb};
use ambermap::markers::parse_sign_payload;

fn main() {
    for world in std::env::args().skip(1) {
        let db = match WorldDb::open(&world) {
            Ok(db) => db,
            Err(e) => {
                println!("skip {world}: {e}");
                continue;
            }
        };
        for dim in [Dimension::Overworld, Dimension::Nether, Dimension::TheEnd] {
            for p in db.block_entity_payloads(dim) {
                let d = ambermap::leveldb::decompress_payload(&p).unwrap_or(p);
                if let Some(sign) = parse_sign_payload(&d) {
                    let text = sign.lines.join(" | ");
                    println!(
                        "{world} dim={dim:?} ({}, {}, {}) : {}",
                        sign.x, sign.y, sign.z, text
                    );
                }
            }
        }
    }
}
