//! Сегмент загрузки

#[derive(Debug, Clone)]
pub struct SegmentInfo {
    pub index: u32,
    pub start_byte: u64,
    pub end_byte: u64,
    pub downloaded: u64,
    pub completed: bool,
}

impl SegmentInfo {
    pub fn new(index: u32, start: u64, end: u64) -> Self {
        Self {
            index,
            start_byte: start,
            end_byte: end,
            downloaded: 0,
            completed: false,
        }
    }

    pub fn length(&self) -> u64 {
        self.end_byte - self.start_byte + 1
    }
}
