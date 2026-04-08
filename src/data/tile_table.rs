use glam::IVec2;

use crate::raw_data::{QuestTile, SpecialTileId};

use super::{Form, Segment, SegmentDef, Terrain};

#[allow(clippy::match_same_arms)]
fn wheat_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2AA_4AF (Normal, BigTree, Granary, Windmill)
        2..=5 => vec![
            (Form::Size2, Terrain::Wheat, 5, 1),
            (Form::Size4, Terrain::Forest, 1, 21),
        ],
        // 2AA
        92 => vec![(Form::Size2, Terrain::Wheat, 0, 1)],
        // 2AA_2AV_1AV
        1 => vec![
            (Form::Size2, Terrain::Wheat, 0, 1),
            (Form::Size2, Terrain::House, 2, 2),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 3AA_1AV (Normal, Granary, Windmill)
        6..=8 => vec![
            (Form::Size3, Terrain::Wheat, 3, 1),
            (Form::Size1, Terrain::House, 0, 1),
        ],
        // 4AA_2AF (Normal, Granary)
        9 | 10 => vec![
            (Form::Size4, Terrain::Wheat, 0, 2),
            (Form::Size2, Terrain::Forest, 4, 7),
        ],
        // 4BA_1AF_1AF (Normal, BigTree)
        11 | 12 => vec![
            (Form::X, Terrain::Wheat, 0, 3),
            (Form::Size1, Terrain::Forest, 2, 1),
            (Form::Size1, Terrain::Forest, 5, 1),
        ],
        // 6AA (Normal, BigTree, Windmill)
        13..=15 => vec![(Form::Size6, Terrain::Wheat, 0, 3)],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn forest_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 1AF (Normal, Deer, Bear, Boar)
        16 | 19 | 65 | 66 => vec![(Form::Size1, Terrain::Forest, 0, 1)],
        // 2AF (Normal, Deer, Bear, Boar)
        67..=70 => vec![(Form::Size2, Terrain::Forest, 0, 2)],
        // 3AF (Normal, Deer, Bear, Boar)
        20 | 21 | 71 | 72 => vec![(Form::Size3, Terrain::Forest, 0, 17)],
        // 4AF (Normal, Ruin)
        22 | 73 => vec![(Form::Size4, Terrain::Forest, 0, 21)],
        // 6AF (Normal, Bear, Boar, Ruin)
        23 | 74 | 75 | 76 => vec![(Form::Size6, Terrain::Forest, 0, 37)],
        // 6AF (Deer)
        24 => vec![(Form::Size6, Terrain::Forest, 0, 26)],
        // 1AF_2AW (Normal, Deer)
        17 | 18 => vec![
            (Form::Size1, Terrain::Forest, 3, 1),
            (Form::Size2, Terrain::Lake, 0, 1),
        ],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn village_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2AV
        33 => vec![(Form::Size2, Terrain::House, 0, 2)],
        // 3AV (Normal, Fountain)
        35 | 38 => vec![(Form::Size3, Terrain::House, 0, 3)],
        // 3AV_3AF (Normal, Fountain, Tower, Fox)
        34 | 36 | 37 | 80 => vec![
            (Form::Size3, Terrain::House, 0, 3),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4BV_1AF_1AF (Normal, Fountain, Tower, Fox)
        39 | 40 | 41 | 84 => vec![
            (Form::FanOut, Terrain::House, 4, 5),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size1, Terrain::Forest, 3, 1),
        ],
        // 5AV_1AF (Normal, Fox)
        85 | 86 => vec![
            (Form::Size5, Terrain::House, 0, 7),
            (Form::Size1, Terrain::Forest, 5, 1),
        ],
        // 6AV (Normal)
        42 => vec![(Form::Size6, Terrain::House, 0, 7)],
        // 6AV (Fountain, Tower)
        43 | 44 => vec![(Form::Size6, Terrain::House, 0, 6)],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn rail_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2BT_3AA_1AA (BUG: says 3AA but tile only has size2 agriculture)
        25 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Wheat, 1, 1),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 2BT_3AF_1AF
        26 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2BT_3AV_1AV
        27 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::House, 1, 1),
            (Form::Size3, Terrain::House, 3, 3),
        ],
        // 2CT_1AF_1AV (Normal, Locomotive)
        28 | 29 => vec![
            (Form::Straight, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size1, Terrain::House, 4, 1),
        ],
        // 2CT (Normal, Locomotive)
        30 | 31 => vec![(Form::Straight, Terrain::Rail, 0, 1)],
        _ => return None,
    })
}

#[allow(clippy::match_same_arms)]
fn water_tile_segments(id: i32) -> Option<Vec<SegmentDef>> {
    Some(match id {
        // 2BW_3AF_1AF (Normal, Boat)
        45 | 46 => vec![
            (Form::Bridge, Terrain::River, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2CW (Normal, Boat, Beaver)
        47 | 54 | 58 => vec![(Form::Straight, Terrain::River, 0, 1)],
        // 2CW_2AA_1AV (Normal)
        49 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Wheat, 1, 1),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 2CW_2AA_2AV (Watermill)
        50 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Wheat, 1, 1),
            (Form::Size2, Terrain::House, 4, 2),
        ],
        // 2CW_2AF_1AA (Normal, Watermill)
        51 | 52 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 2),
            (Form::Size1, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AF_2AA (Normal, Beaver)
        87 | 88 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 2),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AV_1AV
        48 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::House, 1, 2),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 2CW_2AV_2AV_Watermill
        53 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::House, 1, 2),
            (Form::Size2, Terrain::House, 4, 2),
        ],
        // 3AW_3AF (Normal, SwanGoose, Beaver)
        59 | 89 | 60 => vec![
            (Form::Size3, Terrain::Lake, 0, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4AW_2AF (Normal, Beaver, SwanGoose)
        61 | 62 | 90 => vec![
            (Form::Size4, Terrain::Lake, 0, 1),
            (Form::Size2, Terrain::Forest, 4, 2),
        ],
        // 6AW (Normal, Beaver, Ruin, Boat, SwanGoose)
        55 | 63 | 64 | 56 | 91 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        // 6AW_6AT (WaterTrainStation)
        57 => vec![(Form::Size6, Terrain::Station, 0, 1)],
        _ => return None,
    })
}

pub fn raw_segments_for_quest_tile(id: i32) -> Option<Vec<SegmentDef>> {
    wheat_tile_segments(id)
        .or_else(|| forest_tile_segments(id))
        .or_else(|| village_tile_segments(id))
        .or_else(|| rail_tile_segments(id))
        .or_else(|| water_tile_segments(id))
}

/// Get the primary terrain type for a quest tile (the terrain with the most segments).
pub fn quest_terrain(quest_tile_id: i32) -> Option<Terrain> {
    let segments = raw_segments_for_quest_tile(quest_tile_id)?;
    // The first segment's terrain is the quest's target terrain.
    // Lake quests count as River quests (same group).
    segments.first().map(|(_, terrain, _, _)| match terrain {
        Terrain::Lake => Terrain::River,
        t => *t,
    })
}

pub fn segments_from_quest_tile(pos: IVec2, quest_tile: &QuestTile) -> Vec<Segment> {
    let id = quest_tile.quest_tile_id.0;
    let segments = raw_segments_for_quest_tile(id).unwrap_or_else(|| {
        println!("{}\t{}\t=> {}", pos.x, pos.y, id);
        todo!("Unhandled quest tile id {id}");
    });

    segments
        .into_iter()
        .map(|(form, terrain, rotation, unit_count)| Segment {
            pos,
            form,
            terrain,
            rotation,
            unit_count,
        })
        .collect()
}

#[allow(clippy::single_match_else)]
pub fn segments_from_special_tile_id(pos: IVec2, special_tile_id: &SpecialTileId) -> Vec<Segment> {
    match special_tile_id.0 {
        1 => {
            vec![Segment {
                pos,
                form: Form::Size6,
                terrain: Terrain::Station,
                rotation: 0,
                unit_count: 1,
            }]
        }
        other => {
            println!("{}\t{}\t=> {}", pos.x, pos.y, other);
            todo!("Unhandled special tile id {other}");
        }
    }
}
