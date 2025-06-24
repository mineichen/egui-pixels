use std::num::NonZeroU16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubGroup {
    pub position: u32,
    pub length: NonZeroU16,
    // 255 means no other group is associated with these positions
    pub association: u8,
}

impl SubGroup {
    pub fn new(position: u32, length: NonZeroU16, association: u8) -> Self {
        Self {
            position,
            length,
            association,
        }
    }

    pub fn new_total(position: u32, length: NonZeroU16) -> Self {
        Self {
            position,
            length,
            association: 255,
        }
    }

    pub fn as_range(&self) -> std::ops::Range<usize> {
        let start = self.position as usize;
        let end = start + self.length.get() as usize;
        start..end
    }

    pub fn association(&self) -> u8 {
        self.association
    }

    pub fn start_position(&self) -> u32 {
        self.position
    }

    pub fn end_position(&self) -> u32 {
        self.position + self.length.get() as u32
    }
}

pub type SubGroups = Vec<SubGroup>;
