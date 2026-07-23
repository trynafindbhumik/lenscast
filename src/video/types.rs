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

    /// videoflip `method` property value.
    pub fn method(&self) -> &'static str {
        match self {
            Rotation::Original => "none",
            Rotation::Rot90 => "clockwise",
            Rotation::Rot180 => "rotate-180",
            Rotation::Rot270 => "counterclockwise",
        }
    }
}