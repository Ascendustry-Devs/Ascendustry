use crate::log_allocator;

#[derive(Clone)]
pub struct Gap {
    pub position: usize,
    pub length: usize,
}

impl Gap {
    pub const fn new(position: usize, length: usize) -> Self {
        Self { position, length }
    }

    pub const fn start(&self) -> usize {
        self.position
    }

    pub const fn end(&self) -> usize {
        self.position + self.length
    }

    pub const fn shrink_left(&mut self, offset: usize) {
        self.position += offset;
        self.length -= offset;
    }

    pub const fn extend_left(&mut self, offset: usize) {
        self.position -= offset;
        self.length += offset
    }

    pub const fn extend_right(&mut self, offset: usize) {
        self.length += offset
    }
}

pub(super) trait GapManager {
    fn gaps(&self) -> &Vec<Gap>;
    fn gaps_mut(&mut self) -> &mut Vec<Gap>;

    fn find_place(&self, needed: usize) -> Option<usize> {
        log_allocator!("Finding place for {} bytes.", needed);
        self.gaps().iter().position(|x| x.length >= needed)
    }

    /// Returns:
    /// - `Ok`: index of gap with exact [position].
    /// - `Err`: index where a gap with exact [position] can be inserted.
    fn get_gap_index_at(&self, position: usize) -> Result<usize, usize> {
        log_allocator!("Getting gap index at position {}.", position);
        self.gaps().binary_search_by_key(&position, |gap| gap.position)
    }

    /// Inserts `new_gap` at :
    /// - `position` if it's not occupied.
    /// - `position + 1` if it's occupied.
    fn insert_gap_at(&mut self, new_gap: Gap, position: usize) {
        log_allocator!(
            "Inserting gap(pos: {}, len: {}) at position {}.",
            new_gap.position,
            new_gap.length,
            position
        );

        let index = match self.get_gap_index_at(position) {
            Ok(found) => found + 1,
            Err(found) => found,
        };

        self.gaps_mut().insert(index, new_gap);
    }

    fn release_after(&mut self, position: usize, length: usize) {
        let gap = Gap::new(position, length);
        self.insert_gap_at(gap, position);
        self.try_merge_gap(position);
    }

    fn consume_gap(&mut self, gap_index: usize, data_length: usize) -> usize {
        let gap = &mut self.gaps_mut()[gap_index];
        let old_gap_position = gap.start();
        gap.shrink_left(data_length);
        let bytes_left = gap.length;

        if bytes_left == 0 {
            self.gaps_mut().remove(gap_index);
        }

        log_allocator!(
            "Consumed {} bytes of Gap(index: {}), leaving {} bytes.",
            data_length,
            gap_index,
            bytes_left
        );

        old_gap_position
    }

    fn try_merge_gap(&mut self, position: usize) {
        log_allocator!("Trying to merge gap of pos: {}.", position);
        let Ok(index) = self.get_gap_index_at(position) else {
            return;
        };

        self.try_merge_prev_gap(index);
        self.try_merge_next_gap(index);
    }

    fn try_merge_prev_gap(&mut self, index: usize) {
        if index == 0 {
            return;
        }
        let prev = &self.gaps()[index - 1];
        let curr = &self.gaps()[index];
        if prev.end() == curr.start() {
            let offset = prev.length;
            self.gaps_mut()[index].extend_left(offset);
            self.gaps_mut().remove(index - 1);
        }
    }

    fn try_merge_next_gap(&mut self, index: usize) {
        if index + 1 >= self.gaps().len() {
            return;
        }
        let curr = &self.gaps()[index];
        let next = &self.gaps()[index + 1];
        if curr.end() == next.start() {
            let offset = next.length;
            self.gaps_mut()[index].extend_right(offset);
            self.gaps_mut().remove(index + 1);
        }
    }
}
