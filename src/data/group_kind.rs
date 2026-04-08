/// The kind of group a segment can belong to. Unlike `Terrain`, this has no
/// Lake/Station/Empty/Missing variants — those participate in groups via
/// `EdgeTerrain::group_memberships()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroupKind {
    House,
    Forest,
    Wheat,
    Rail,
    River,
}

impl GroupKind {
    /// Does this group kind accept segments of the given terrain?
    pub fn accepts(self, terrain: super::Terrain) -> bool {
        use super::Terrain;
        matches!(
            (self, terrain),
            (GroupKind::House, Terrain::House)
                | (GroupKind::Forest, Terrain::Forest)
                | (GroupKind::Wheat, Terrain::Wheat)
                | (GroupKind::Rail, Terrain::Rail)
                | (GroupKind::Rail, Terrain::Station)
                | (GroupKind::River, Terrain::River)
                | (GroupKind::River, Terrain::Lake)
                | (GroupKind::River, Terrain::Station)
        )
    }

    /// All group kinds a terrain can belong to.
    pub fn memberships_of(terrain: super::Terrain) -> &'static [GroupKind] {
        use super::Terrain;
        match terrain {
            Terrain::House => &[GroupKind::House],
            Terrain::Forest => &[GroupKind::Forest],
            Terrain::Wheat => &[GroupKind::Wheat],
            Terrain::Rail => &[GroupKind::Rail],
            Terrain::River => &[GroupKind::River],
            Terrain::Lake => &[GroupKind::River],
            Terrain::Station => &[GroupKind::River, GroupKind::Rail],
            Terrain::Empty | Terrain::Missing => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Terrain;
    use super::*;

    #[test]
    fn test_memberships_matches_accepts() {
        // For every (GroupKind, Terrain) pair, memberships_of and accepts must agree.
        let all_kinds = [
            GroupKind::House,
            GroupKind::Forest,
            GroupKind::Wheat,
            GroupKind::Rail,
            GroupKind::River,
        ];
        let all_terrains = [
            Terrain::House,
            Terrain::Forest,
            Terrain::Wheat,
            Terrain::Rail,
            Terrain::River,
            Terrain::Lake,
            Terrain::Station,
            Terrain::Empty,
            Terrain::Missing,
        ];
        for kind in all_kinds {
            for terrain in all_terrains {
                let from_accepts = kind.accepts(terrain);
                let from_memberships = GroupKind::memberships_of(terrain).contains(&kind);
                assert_eq!(
                    from_accepts, from_memberships,
                    "Mismatch: {kind:?}.accepts({terrain:?})={from_accepts}, memberships={from_memberships}"
                );
            }
        }
    }

    #[test]
    fn test_station_in_two_groups() {
        let m = GroupKind::memberships_of(Terrain::Station);
        assert_eq!(m.len(), 2);
        assert!(m.contains(&GroupKind::River));
        assert!(m.contains(&GroupKind::Rail));
    }

    #[test]
    fn test_lake_in_river_group() {
        let m = GroupKind::memberships_of(Terrain::Lake);
        assert_eq!(m, &[GroupKind::River]);
    }

    #[test]
    fn test_empty_in_no_groups() {
        assert!(GroupKind::memberships_of(Terrain::Empty).is_empty());
        assert!(GroupKind::memberships_of(Terrain::Missing).is_empty());
    }

    #[test]
    fn test_accepts_consistency_with_extends_group_of() {
        // Verify GroupKind::accepts matches the existing Terrain::extends_group_of
        // for all cases where extends_group_of doesn't panic.
        let group_terrains = [
            (GroupKind::House, Terrain::House),
            (GroupKind::Forest, Terrain::Forest),
            (GroupKind::Wheat, Terrain::Wheat),
            (GroupKind::Rail, Terrain::Rail),
            (GroupKind::River, Terrain::River),
        ];
        let member_terrains = [
            Terrain::House,
            Terrain::Forest,
            Terrain::Wheat,
            Terrain::Rail,
            Terrain::River,
            Terrain::Lake,
            Terrain::Station,
        ];
        for (kind, group_terrain) in group_terrains {
            for member in member_terrains {
                let old = member.extends_group_of(group_terrain);
                let new = kind.accepts(member);
                assert_eq!(
                    old, new,
                    "Mismatch: {member:?}.extends_group_of({group_terrain:?})={old}, {kind:?}.accepts({member:?})={new}"
                );
            }
        }
    }
}
