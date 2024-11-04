// use glam::IVec2;
//
// use crate::data::{Tile, TileId};
//
// type IndexKey = usize;
//
// #[derive(Default)]
// pub struct Index {
//     offset: IVec2,
//     size: IVec2,
//     index: Vec<Option<[SegmentId;6]>>,
// }
//
// impl Index {
//     pub fn offset_and_size(&self) -> (IVec2, IVec2) {
//         (self.offset, self.size)
//     }
//
//     pub fn index_data(&self) -> &[Option<TileId>] {
//         &self.index
//     }
//
//     /// Compute the position of tile at `pos` in the index structure.
//     pub fn tile_key(&self, pos: IVec2) -> Option<IndexKey> {
//         let upper = self.offset + self.size;
//         let valid_s = pos.x >= self.offset.x && pos.x < upper.x;
//         let valid_t = pos.y >= self.offset.y && pos.y < upper.y;
//         (valid_s && valid_t).then(|| {
//             IndexKey::try_from((pos.y - self.offset.y) * self.size.x + (pos.x - self.offset.x))
//                 .unwrap()
//         })
//     }
//
//     fn key_value(&self, key: IndexKey) -> Option<TileId> {
//         self.index.get(key).copied().flatten()
//     }
//
//     /// Compute the position of tile at `pos` in the tiles' list.
//     pub fn tile_index(&self, pos: IVec2) -> Option<TileId> {
//         self.tile_key(pos).and_then(|key| self.key_value(key))
//     }
//
//     pub fn from(tiles: &[Tile]) -> Self {
//         let (min, max) = tiles.iter().fold(
//             (
//                 IVec2::new(i32::MAX, i32::MAX),
//                 IVec2::new(i32::MIN, i32::MIN),
//             ),
//             |(min, max), tile| {
//                 (
//                     IVec2::new(min.x.min(tile.pos.x), min.y.min(tile.pos.y)),
//                     IVec2::new(max.x.max(tile.pos.x), max.y.max(tile.pos.y)),
//                 )
//             },
//         );
//
//         let lower = min - IVec2::ONE;
//         let upper = max + IVec2::ONE;
//         let size = upper - lower + IVec2::ONE;
//         let mut index = Self {
//             offset: lower,
//             size,
//             index: vec![None; usize::try_from(size.x * size.y).unwrap()],
//         };
//
//         tiles.iter().enumerate().for_each(|(tile_index, tile)| {
//             let key = index.tile_key(tile.pos).unwrap();
//             index.index[key] = Some(tile_index);
//         });
//
//         index
//     }
// }
