use std::usize;

use wgpu::{BufferUsages, CommandEncoder, Device, Queue};

use crate::engine::render::{mesh::mesh::MeshId, utils::smart_buffer::SmartBuffer};

pub struct DataEntry<'a> {
    pub id: MeshId,
    pub data: &'a [u8],
}

impl<'a> DataEntry<'a> {
    pub fn new(id: MeshId, data: &'a [u8]) -> Self {
        Self { id, data }
    }
}

pub struct MeshEntry {
    pub id: MeshId,
    pub position: usize,
    pub length: usize,
}

#[derive(Clone)]
pub struct Gap {
    pub position: usize,
    pub length: usize,
}

pub struct MeshManager {
    pub buffer: SmartBuffer,
    pub data: Vec<MeshEntry>,
    gaps: Vec<Gap>,
    defrag_strategy: DefragmentationStrategy,
    pending_destruction: Vec<SmartBuffer>,
    next_id: MeshId,
}

enum DefragmentationStrategy {
    EventBased,
    // TimeBased,
    // LastResort,
}

impl Gap {
    pub fn new(position: usize, length: usize) -> Self {
        Self { position, length }
    }
}

impl MeshManager {
    pub fn new(device: &Device) -> Self {
        Self {
            buffer: SmartBuffer::from_capacity(
                0,
                device,
                None,
                BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX,
            ),
            data: vec![],
            gaps: vec![],
            pending_destruction: vec![],
            next_id: 0,
            defrag_strategy: DefragmentationStrategy::EventBased,
        }
    }

    fn reallocate_defragment(&mut self, device: &Device, encoder: &mut CommandEncoder, needed: usize) {
        // println!("Reallocate + defragment - needed: {}", needed);

        // Reallocate
        let new_buffer = SmartBuffer::from_capacity(
            needed as u32,
            device,
            None,
            BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX,
        );

        let mut current_position = 0;

        // Copy each entry to the new buffer without any gaps
        for entry in &mut self.data {
            // println!("entry? hello?");
            // println!(
            //     "Entry pos/length/current_pos: {} {} {}",
            //     entry.position, entry.length, current_position
            // );
            encoder.copy_buffer_to_buffer(
                self.buffer.buffer(),
                entry.position as u64,
                new_buffer.buffer(),
                current_position as u64,
                entry.length as u64,
            );

            // Update entry
            entry.position = current_position;

            current_position += entry.length;
        }

        // Update gaps
        self.gaps.clear();
        self.gaps
            .push(Gap::new(current_position, new_buffer.capacity() as usize - current_position));

        // Update buffer
        self.pending_destruction.push(std::mem::replace(&mut self.buffer, new_buffer));
    }

    fn find_place(&self, needed: usize) -> Option<usize> {
        self.gaps.iter().position(|x| x.length >= needed)
    }

    fn get_data_next_gap(&self, entry: &MeshEntry) -> Option<usize> {
        let position = entry.position + entry.length;
        self.gaps.iter().position(|gap| gap.position == position)
    }

    fn write_at(&mut self, queue: &Queue, position: usize, data: &[u8]) {
        queue.write_buffer(self.buffer.buffer(), position as u64, data);
    }

    fn insert_gap_after(&mut self, gap: Gap, position: usize) {
        let gap_index = self.gaps.iter().position(|x| x.position > position).unwrap_or(self.gaps.len());

        self.gaps.insert(gap_index, gap);
    }

    pub fn process_pending_destructions(&mut self) {
        for mut buf in self.pending_destruction.drain(..) {
            buf.destroy();
        }
    }

    fn consume_gap_for(&mut self, queue: &Queue, gap_index: usize, entry: &DataEntry) {
        let position = self.gaps[gap_index].position;
        let data_length = entry.data.len();

        self.write_at(queue, position, entry.data);

        let gap = &mut self.gaps[gap_index];
        gap.position += data_length;
        gap.length -= data_length;

        if gap.length == 0 {
            self.gaps.remove(gap_index);
        }
    }

    pub fn update_data(&mut self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, entry: DataEntry) {
        let Some(index) = self.data.iter().position(|x| x.id == entry.id) else {
            return;
        };

        let (position, old_len) = {
            let entry = &self.data[index];
            (entry.position, entry.length)
        };
        let new_len = entry.data.len();

        // Cas 1: les nouvelles données ont une taille inférieure ou égale aux précédentes
        if new_len <= old_len {
            self.write_at(queue, position, entry.data);

            if new_len < old_len {
                if let Some(gap_index) = self.get_data_next_gap(&self.data[index]) {
                    let gap = &mut self.gaps[gap_index];

                    let delta_length = old_len - new_len;
                    gap.position -= delta_length;
                    gap.length += delta_length;
                } else {
                    let gap = Gap::new(position + new_len, old_len - new_len);
                    self.insert_gap_after(gap, position);
                }
            }

            self.data[index].length = new_len;

            return;
        }

        // Cas 2: on regarde si la taille des anciennes données + le trou qui les suit est suffisant pour accueillir les nouvelles données
        let gap_index = self.gaps.iter().position(|x| x.position == position + old_len);

        // S'il y a un trou après
        if let Some(gap_index) = gap_index {
            let gap_length = self.gaps[gap_index].length;

            // Si taille(anciennes données + trou) suffisent
            if new_len <= old_len + gap_length {
                self.write_at(queue, position, entry.data);

                let gap = &mut self.gaps[gap_index];
                gap.position = position + new_len;
                gap.length = old_len + gap.length - new_len;

                if gap.length == 0 {
                    self.gaps.remove(gap_index);
                }

                self.data[index].length = new_len;

                return;
            }
            // Si ça suffit pas, on élargit le trou à (ancienne données + trou actuel) et donc on marque l'emplacement comme libre
            else {
                let gap = &mut self.gaps[gap_index];
                gap.position = position;
                gap.length += old_len;

                self.data.remove(index);
            }
        }
        // S'il n'y a pas de trou après les données actuelle, on a pas la place pour stocker les nouvelles. On marque alors l'emplacement actuel comme libre
        else {
            let gap = Gap::new(position, old_len);
            self.insert_gap_after(gap, position);

            self.data.remove(index);
        }

        // Cas 3: on regarde s'il existe un trou suffisant pour accueillir les nouvelles données...
        if let Some(gap_index) = self.find_place(new_len) {
            self.consume_gap_for(queue, gap_index, &entry);
            return;
        }

        // Cas 4: c'est la merde, donc on réalloue et défragmente pour avoir assez d'espace pour les nouvelles données (DERNIER RECOURS)
        let needed = self.data.iter().fold(0, |acc, x| acc + x.length) + new_len;
        self.reallocate_defragment(device, encoder, needed);

        let gap_index = self
            .find_place(new_len)
            .expect("Failed to update data. No free space found even after reallocation.");
        self.consume_gap_for(queue, gap_index, &entry);
    }

    pub fn add_data(&mut self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, data: &[u8]) -> Option<MeshId> {
        let mut index = self.find_place(data.len());

        if index.is_none() {
            let needed = self.data.iter().fold(0, |acc, x| acc + x.length) + data.len();
            self.reallocate_defragment(device, encoder, needed);

            index = self.find_place(data.len());
        }

        let index = index.expect("No space found, even after re-allocation.");

        let (position, gap_length) = {
            let gap = &mut self.gaps[index];

            let pos = gap.position;
            let len = gap.length;

            gap.position += data.len();
            gap.length -= data.len();

            (pos, len)
        };

        // println!(
        //     "add_data {} {} {} {} {}",
        //     position,
        //     data.len(),
        //     gap_length,
        //     self.buffer.capacity(),
        //     self.buffer.length()
        // );
        self.write_at(queue, position, data);

        if gap_length == 0 {
            self.gaps.remove(index);
        }

        let id = self.next_id;
        self.next_id += 1;

        let entry = MeshEntry {
            id: id,
            position: position,
            length: data.len(),
        };
        let entry_index = self
            .data
            .iter()
            .position(|x| x.position > entry.position)
            .unwrap_or(self.data.len());
        self.data.insert(entry_index, entry);

        Some(id)
    }

    fn try_merge_gap(&mut self, position: usize) {
        if let Some(current_index) = self.gaps.iter().position(|x| x.position == position) {
            let current_length = self.gaps[current_index].length;
            if let Some(next_index) = self.gaps.iter().position(|x| x.position == position + current_length) {
                if next_index <= current_index {
                    return;
                }
                let next = self.gaps.remove(next_index);
                self.gaps[current_index].length += next.length;
            };
        };
    }

    pub fn free_data(&mut self, id: MeshId) {
        let Some(data_index) = self.data.iter().position(|x| x.id == id) else {
            return;
        };

        let entry = &self.data[data_index];
        let (position, length) = (entry.position, entry.length);

        let gap = Gap::new(position, length);

        self.insert_gap_after(gap, position);
        self.try_merge_gap(position);

        self.data.remove(data_index);
    }

    fn merge_gaps(&mut self) {
        if self.gaps.is_empty() {
            return;
        }

        self.gaps.sort_by_key(|g| g.position);

        let mut merged: Vec<Gap> = Vec::with_capacity(self.gaps.len());

        for gap in self.gaps.drain(..) {
            if let Some(last) = merged.last_mut() {
                if last.position + last.length == gap.position {
                    last.length += gap.length;
                    continue;
                }
            }
            merged.push(gap);
        }

        self.gaps = merged;
    }
}
