#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Original,
    Rot90,
    Rot180,
    Rot270,
}

impl Rotation {
    pub fn from_index(i: u8) -> Self {
        match i {
            1 => Rotation::Rot90,
            2 => Rotation::Rot180,
            3 => Rotation::Rot270,
            _ => Rotation::Original,
        }
    }

    #[allow(dead_code)]
    pub fn method(&self) -> &'static str {
        match self {
            Rotation::Original => "none",
            Rotation::Rot90 => "clockwise",
            Rotation::Rot180 => "rotate-180",
            Rotation::Rot270 => "counterclockwise",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_from_index_maps_correctly() {
        assert_eq!(Rotation::from_index(0), Rotation::Original);
        assert_eq!(Rotation::from_index(1), Rotation::Rot90);
        assert_eq!(Rotation::from_index(2), Rotation::Rot180);
        assert_eq!(Rotation::from_index(3), Rotation::Rot270);
    }

    #[test]
    fn rotation_from_index_out_of_range_defaults_to_original() {
        assert_eq!(Rotation::from_index(4), Rotation::Original);
        assert_eq!(Rotation::from_index(255), Rotation::Original);
        assert_eq!(Rotation::from_index(u8::MAX), Rotation::Original);
    }
}
