use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dorfromantische2_rs::best_placements::BestPlacements;
use dorfromantische2_rs::group_assignments::GroupAssignments;
use dorfromantische2_rs::map::Map;
use dorfromantische2_rs::raw_data::SaveGame;
use std::io::Cursor;

fn load_biggame_bytes() -> Vec<u8> {
    std::fs::read("tests/fixtures/biggame.sav").expect("tests/fixtures/biggame.sav not found")
}

fn parse_nrbf(data: &[u8]) -> nrbf_rs::value::Value {
    nrbf_rs::parse_nrbf(&mut Cursor::new(data))
}

fn bench_parse_nrbf(c: &mut Criterion) {
    let data = load_biggame_bytes();
    c.bench_function("parse_nrbf (biggame)", |b| {
        b.iter(|| parse_nrbf(black_box(&data)))
    });
}

fn bench_savegame_from_value(c: &mut Criterion) {
    let data = load_biggame_bytes();
    let value = parse_nrbf(&data);
    c.bench_function("SaveGame::try_from (biggame)", |b| {
        b.iter(|| SaveGame::try_from(black_box(&value)).unwrap())
    });
}

fn bench_map_from_savegame(c: &mut Criterion) {
    let data = load_biggame_bytes();
    let value = parse_nrbf(&data);
    let savegame = SaveGame::try_from(&value).unwrap();
    c.bench_function("Map::from (biggame)", |b| {
        b.iter(|| Map::from(black_box(&savegame)))
    });
}

fn bench_group_assignments(c: &mut Criterion) {
    let data = load_biggame_bytes();
    let value = parse_nrbf(&data);
    let savegame = SaveGame::try_from(&value).unwrap();
    let map = Map::from(&savegame);
    c.bench_function("GroupAssignments::from (biggame)", |b| {
        b.iter(|| GroupAssignments::from(black_box(&map)))
    });
}

fn bench_best_placements(c: &mut Criterion) {
    let data = load_biggame_bytes();
    let value = parse_nrbf(&data);
    let savegame = SaveGame::try_from(&value).unwrap();
    let map = Map::from(&savegame);
    let groups = GroupAssignments::from(&map);
    let freqs = dorfromantische2_rs::tile_frequency::TileFrequencies::from_map(&map);
    c.bench_function("BestPlacements::compute (biggame)", |b| {
        b.iter(|| BestPlacements::compute(black_box(&map), black_box(&groups), black_box(&freqs)))
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let data = load_biggame_bytes();
    c.bench_function("full pipeline (biggame)", |b| {
        b.iter(|| {
            let value = parse_nrbf(black_box(&data));
            let savegame = SaveGame::try_from(&value).unwrap();
            let map = Map::from(&savegame);
            let groups = GroupAssignments::from(&map);
            let freqs = dorfromantische2_rs::tile_frequency::TileFrequencies::from_map(&map);
            let _placements = BestPlacements::compute(&map, &groups, &freqs);
        })
    });
}

criterion_group!(
    benches,
    bench_parse_nrbf,
    bench_savegame_from_value,
    bench_map_from_savegame,
    bench_group_assignments,
    bench_best_placements,
    bench_full_pipeline,
);
criterion_main!(benches);
