use crate::raw_data::{QuestTile, QuestTileId, SpecialTileId};

use super::{Form, HexPos, Segment, SegmentDef, Terrain};

fn wheat_tile_segments(id: QuestTileId) -> Option<Vec<SegmentDef>> {
    Some(match id.0 {
        // 2AA_2AV_1AV
        1 => vec![
            (Form::Size2, Terrain::Wheat, 0, 1),
            (Form::Size2, Terrain::House, 2, 2),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 2AA_4AF (Normal)
        2 => vec![
            (Form::Size2, Terrain::Wheat, 5, 1),
            (Form::Size4, Terrain::Forest, 1, 17),
        ],
        // 2AA_4AF (BigTree)
        3 => vec![
            (Form::Size2, Terrain::Wheat, 5, 1),
            (Form::Size4, Terrain::Forest, 1, 17),
        ],
        // 2AA_4AF (Granary)
        4 => vec![
            (Form::Size2, Terrain::Wheat, 5, 1),
            (Form::Size4, Terrain::Forest, 1, 17),
        ],
        // 2AA_4AF (Windmill)
        5 => vec![
            (Form::Size2, Terrain::Wheat, 5, 1),
            (Form::Size4, Terrain::Forest, 1, 17),
        ],
        // 3AA_1AV (Normal)
        6 => vec![
            (Form::Size3, Terrain::Wheat, 3, 1),
            (Form::Size1, Terrain::House, 0, 1),
        ],
        // 3AA_1AV (Granary)
        7 => vec![
            (Form::Size3, Terrain::Wheat, 3, 1),
            (Form::Size1, Terrain::House, 0, 1),
        ],
        // 3AA_1AV (Windmill)
        8 => vec![
            (Form::Size3, Terrain::Wheat, 3, 1),
            (Form::Size1, Terrain::House, 0, 1),
        ],
        // 4AA_2AF (Normal)
        9 => vec![
            (Form::Size4, Terrain::Wheat, 0, 2),
            (Form::Size2, Terrain::Forest, 4, 7),
        ],
        // 4AA_2AF (Granary)
        10 => vec![
            (Form::Size4, Terrain::Wheat, 0, 2),
            (Form::Size2, Terrain::Forest, 4, 7),
        ],
        // 4BA_1AF_1AF (Normal)
        11 => vec![
            (Form::X, Terrain::Wheat, 0, 3),
            (Form::Size1, Terrain::Forest, 2, 4),
            (Form::Size1, Terrain::Forest, 5, 4),
        ],
        // 4BA_1AF_1AF (BigTree)
        12 => vec![
            (Form::X, Terrain::Wheat, 0, 3),
            (Form::Size1, Terrain::Forest, 2, 4),
            (Form::Size1, Terrain::Forest, 5, 4),
        ],
        // 6AA (Normal)
        13 => vec![(Form::Size6, Terrain::Wheat, 0, 3)],
        // 6AA (BigTree)
        14 => vec![(Form::Size6, Terrain::Wheat, 0, 3)],
        // 6AA (Windmill)
        15 => vec![(Form::Size6, Terrain::Wheat, 0, 3)],
        // 2AA
        92 => vec![(Form::Size2, Terrain::Wheat, 0, 1)],
        _ => return None,
    })
}

fn forest_tile_segments(id: QuestTileId) -> Option<Vec<SegmentDef>> {
    Some(match id.0 {
        // 1AF (Normal)
        16 => vec![(Form::Size1, Terrain::Forest, 0, 4)],
        // 1AF_2AW (Normal)
        17 => vec![
            (Form::Size1, Terrain::Forest, 3, 4),
            (Form::Size2, Terrain::Lake, 0, 1),
        ],
        // 1AF_2AW (Deer)
        18 => vec![
            (Form::Size1, Terrain::Forest, 3, 4),
            (Form::Size2, Terrain::Lake, 0, 1),
        ],
        // 1AF (Deer)
        19 => vec![(Form::Size1, Terrain::Forest, 0, 4)],
        // 3AF (Normal)
        20 => vec![(Form::Size3, Terrain::Forest, 0, 17)],
        // 3AF (Deer)
        21 => vec![(Form::Size3, Terrain::Forest, 0, 17)],
        // 4AF (Normal)
        22 => vec![(Form::Size4, Terrain::Forest, 0, 21)],
        // 6AF (Normal)
        23 => vec![(Form::Size6, Terrain::Forest, 0, 33)],
        // 6AF (Deer)
        24 => vec![(Form::Size6, Terrain::Forest, 0, 26)],
        // 1AF (Bear)
        65 => vec![(Form::Size1, Terrain::Forest, 0, 4)],
        // 1AF (Boar)
        66 => vec![(Form::Size1, Terrain::Forest, 0, 4)],
        // 2AF (Normal)
        67 => vec![(Form::Size2, Terrain::Forest, 0, 10)],
        // 2AF (Deer)
        68 => vec![(Form::Size2, Terrain::Forest, 0, 10)],
        // 2AF (Bear)
        69 => vec![(Form::Size2, Terrain::Forest, 0, 10)],
        // 2AF (Boar)
        70 => vec![(Form::Size2, Terrain::Forest, 0, 10)],
        // 3AF (Bear)
        71 => vec![(Form::Size3, Terrain::Forest, 0, 17)],
        // 3AF (Boar)
        72 => vec![(Form::Size3, Terrain::Forest, 0, 17)],
        // 4AF (Ruin)
        73 => vec![(Form::Size4, Terrain::Forest, 0, 21)],
        // 6AF (Bear)
        74 => vec![(Form::Size6, Terrain::Forest, 0, 26)],
        // 6AF (Boar)
        75 => vec![(Form::Size6, Terrain::Forest, 0, 37)],
        // 6AF (Ruin)
        76 => vec![(Form::Size6, Terrain::Forest, 0, 37)],
        _ => return None,
    })
}

fn village_tile_segments(id: QuestTileId) -> Option<Vec<SegmentDef>> {
    Some(match id.0 {
        // 2AV
        33 => vec![(Form::Size2, Terrain::House, 0, 2)],
        // 3AV_3AF (Normal)
        34 => vec![
            (Form::Size3, Terrain::House, 0, 3),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 3AV (Normal)
        35 => vec![(Form::Size3, Terrain::House, 0, 3)],
        // 3AV_3AF (Fountain)
        36 => vec![
            (Form::Size3, Terrain::House, 0, 3),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 3AV_3AF (Tower)
        37 => vec![
            (Form::Size3, Terrain::House, 0, 3),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 3AV (Fountain)
        38 => vec![(Form::Size3, Terrain::House, 0, 3)],
        // 4BV_1AF_1AF (Normal)
        39 => vec![
            (Form::FanOut, Terrain::House, 4, 5),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size1, Terrain::Forest, 3, 4),
        ],
        // 4BV_1AF_1AF (Fountain)
        40 => vec![
            (Form::FanOut, Terrain::House, 4, 5),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size1, Terrain::Forest, 3, 4),
        ],
        // 4BV_1AF_1AF (Tower)
        41 => vec![
            (Form::FanOut, Terrain::House, 4, 5),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size1, Terrain::Forest, 3, 4),
        ],
        // 6AV (Normal)
        42 => vec![(Form::Size6, Terrain::House, 0, 7)],
        // 6AV (Fountain)
        43 => vec![(Form::Size6, Terrain::House, 0, 6)],
        // 6AV (Tower)
        44 => vec![(Form::Size6, Terrain::House, 0, 6)],
        // 3AV_3AF (Fox)
        80 => vec![
            (Form::Size3, Terrain::House, 0, 3),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4BV_1AF_1AF (Fox)
        84 => vec![
            (Form::FanOut, Terrain::House, 4, 5),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size1, Terrain::Forest, 3, 4),
        ],
        // 5AV_1AF (Normal)
        85 => vec![
            (Form::Size5, Terrain::House, 0, 7),
            (Form::Size1, Terrain::Forest, 5, 4),
        ],
        // 5AV_1AF (Fox)
        86 => vec![
            (Form::Size5, Terrain::House, 0, 7),
            (Form::Size1, Terrain::Forest, 5, 4),
        ],
        _ => return None,
    })
}

fn rail_tile_segments(id: QuestTileId) -> Option<Vec<SegmentDef>> {
    Some(match id.0 {
        // 2BT_3AA_1AA
        25 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Wheat, 1, 1),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 2BT_3AF_1AF
        26 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2BT_3AV_1AV
        27 => vec![
            (Form::Bridge, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::House, 1, 1),
            (Form::Size3, Terrain::House, 3, 3),
        ],
        // 2CT_1AF_1AV (Normal)
        28 => vec![
            (Form::Straight, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size1, Terrain::House, 4, 1),
        ],
        // 2CT_1AF_1AV (Locomotive)
        29 => vec![
            (Form::Straight, Terrain::Rail, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size1, Terrain::House, 4, 1),
        ],
        // 2CT (Normal)
        30 => vec![(Form::Straight, Terrain::Rail, 0, 1)],
        // 2CT (Locomotive)
        31 => vec![(Form::Straight, Terrain::Rail, 0, 1)],
        _ => return None,
    })
}

fn water_tile_segments(id: QuestTileId) -> Option<Vec<SegmentDef>> {
    Some(match id.0 {
        // 2BW_3AF_1AF (Normal)
        45 => vec![
            (Form::Bridge, Terrain::River, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2BW_3AF_1AF (Boat)
        46 => vec![
            (Form::Bridge, Terrain::River, 0, 1),
            (Form::Size1, Terrain::Forest, 1, 4),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 2CW (Normal)
        47 => vec![(Form::Straight, Terrain::River, 0, 1)],
        // 2CW_2AV_1AV
        48 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::House, 1, 2),
            (Form::Size1, Terrain::House, 5, 1),
        ],
        // 2CW_2AA_1AV
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
        // 2CW_2AF_1AA (Normal)
        51 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 10),
            (Form::Size1, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AF_1AA (Watermill)
        52 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 10),
            (Form::Size1, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AV_2AV_Watermill
        53 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::House, 1, 2),
            (Form::Size2, Terrain::House, 4, 2),
        ],
        // 2CW (Boat)
        54 => vec![(Form::Straight, Terrain::River, 0, 1)],
        // 6AW (Normal)
        55 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        // 6AW (Boat)
        56 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        // 6AW_6AT (WaterTrainStation)
        57 => vec![(Form::Size6, Terrain::Station, 0, 1)],
        // 2CW (Beaver)
        58 => vec![(Form::Straight, Terrain::River, 0, 1)],
        // 3AW_3AF (Normal)
        59 => vec![
            (Form::Size3, Terrain::Lake, 0, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 3AW_3AF (SwanGoose)
        60 => vec![
            (Form::Size3, Terrain::Lake, 0, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4AW_2AF (Normal)
        61 => vec![
            (Form::Size4, Terrain::Lake, 0, 1),
            (Form::Size2, Terrain::Forest, 4, 10),
        ],
        // 4AW_2AF (Beaver)
        62 => vec![
            (Form::Size4, Terrain::Lake, 0, 1),
            (Form::Size2, Terrain::Forest, 4, 10),
        ],
        // 6AW (Beaver)
        63 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        // 6AW (Ruin)
        64 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        // 2CW_2AF_2AA (Normal)
        87 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 10),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 2CW_2AF_2AA (Beaver)
        88 => vec![
            (Form::Straight, Terrain::River, 0, 1),
            (Form::Size2, Terrain::Forest, 1, 10),
            (Form::Size2, Terrain::Wheat, 4, 1),
        ],
        // 3AW_3AF (Beaver)
        89 => vec![
            (Form::Size3, Terrain::Lake, 0, 1),
            (Form::Size3, Terrain::Forest, 3, 17),
        ],
        // 4AW_2AF (SwanGoose)
        90 => vec![
            (Form::Size4, Terrain::Lake, 0, 1),
            (Form::Size2, Terrain::Forest, 4, 10),
        ],
        // 6AW (SwanGoose)
        91 => vec![(Form::Size6, Terrain::Lake, 0, 1)],
        _ => return None,
    })
}

pub fn raw_segments_for_quest_tile(id: QuestTileId) -> Option<Vec<SegmentDef>> {
    wheat_tile_segments(id)
        .or_else(|| forest_tile_segments(id))
        .or_else(|| village_tile_segments(id))
        .or_else(|| rail_tile_segments(id))
        .or_else(|| water_tile_segments(id))
}

/// Get the primary terrain type for a quest tile (the terrain with the most segments).
pub fn quest_terrain(quest_tile_id: QuestTileId) -> Option<Terrain> {
    let segments = raw_segments_for_quest_tile(quest_tile_id)?;
    // The first segment's terrain is the quest's target terrain.
    // Lake quests count as River quests (same group).
    segments.first().map(|(_, terrain, _, _)| match terrain {
        Terrain::Lake => Terrain::River,
        t => *t,
    })
}

pub fn segments_from_quest_tile(pos: HexPos, quest_tile: &QuestTile) -> Vec<Segment> {
    let id = quest_tile.quest_tile_id;
    let segments = match raw_segments_for_quest_tile(id) {
        Some(s) => s,
        None => {
            log::warn!(
                "Unhandled quest tile id {} at ({}, {}), returning empty segments",
                id.0,
                pos.x(),
                pos.y()
            );
            return Vec::new();
        }
    };

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

pub fn segments_from_special_tile_id(pos: HexPos, special_tile_id: &SpecialTileId) -> Vec<Segment> {
    if special_tile_id.0 == 1 {
        vec![Segment {
            pos,
            form: Form::Size6,
            terrain: Terrain::Station,
            rotation: 0,
            unit_count: 1,
        }]
    } else {
        let other = special_tile_id.0;
        log::warn!(
            "Unhandled special tile id {other} at ({}, {}), returning empty segments",
            pos.x(),
            pos.y()
        );
        Vec::new()
    }
}
