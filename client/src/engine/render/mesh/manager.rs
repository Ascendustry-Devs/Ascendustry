use wgpu::{Buffer, BufferUsages, CommandEncoder, Device, Queue};

use crate::engine::render::utils::smart_buffer::SmartBuffer;

const BYTES_PER_FRAME_CAP: usize = 1024 * 1024 * 8;
const MAX_MILLIS_PER_FRAME_CAP: u128 = 8;
const MAX_WRITE_OPERATIONS_PER_FRAME: usize = 5;
const ARENA_MIN_SIZE: usize = 1024 * 1024 * 16;

pub type MeshId = u32;

pub struct DataEntry<'a> {
    pub id: MeshId,
    pub data: &'a [u8],
}

impl<'a> DataEntry<'a> {
    pub fn new(id: MeshId, data: &'a [u8]) -> Self {
        Self { id, data }
    }
}

#[repr(u8)]
pub enum AllocError {
    InvalidId = 0,
    NotEnoughSpace = 1,
}

pub struct MeshEntry {
    pub id: MeshId,
    pub position: usize,
    pub length: usize,
}

#[derive(Clone)]
struct Gap {
    pub position: usize,
    pub length: usize,
}

struct WriteOperation {
    mesh_id: MeshId,
    offset: usize,
    len: usize,
    arena_offset: usize,
}

pub struct MeshManager {
    buffer: SmartBuffer,
    pub data: Vec<MeshEntry>,
    gaps: Vec<Gap>,
    pending_destruction: Vec<SmartBuffer>,
    next_id: MeshId,
    free_ids: Vec<MeshId>,
    write_operations: Vec<WriteOperation>,
    schedule_batch: bool,
    arena: Vec<u8>,
}

impl Gap {
    pub fn new(position: usize, length: usize) -> Self {
        Self { position, length }
    }
}

/// Convertit [b] en Mb et Kb.
/// Retourne (Mb, Kb, b).
pub fn bytes_conversion(b: u32) -> (u32, u32, u32) {
    let kb = b / 1024;
    let mb = kb / 1024;
    return (mb, kb, b);
}

impl MeshManager {
    pub fn new(device: &Device) -> Self {
        let mut arena = Vec::new();
        arena.reserve(ARENA_MIN_SIZE);
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
            free_ids: Vec::with_capacity(64),
            write_operations: vec![],
            schedule_batch: false,
            arena: arena,
        }
    }

    pub fn print_buffer_memory_usage(&self) {
        let data_length = self.total_data_length() as u32;
        let (len_mb, len_kb, len_b) = bytes_conversion(data_length);
        let (cap_mb, cap_kb, cap_b) = bytes_conversion(self.buffer.capacity());
        println!(
            "Mesh buffer\nLen: {:3}Mb | {:6}Kb | {:9}b\nCap: {:3}Mb | {:6}Kb | {:9}b",
            len_mb, len_kb, len_b, cap_mb, cap_kb, cap_b,
        );
    }

    pub fn get_buffer(&self) -> &Buffer {
        self.buffer.buffer()
    }

    fn total_data_length(&self) -> usize {
        self.data.iter().fold(0, |acc, v| acc + v.length)
    }

    fn reallocate_defragment(&mut self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, needed: usize) {
        println!(
            "Reallocate and defragment because current buffer has {} bytes of capacity but we need {} bytes.",
            self.total_data_length(),
            needed
        );
        if !self.write_operations.is_empty() {
            self.flush(queue);
        }

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
        println!(
            "new realloc gap: {} {}",
            current_position,
            new_buffer.capacity() as usize - current_position
        );
        self.gaps
            .push(Gap::new(current_position, new_buffer.capacity() as usize - current_position));

        // Update buffer
        self.pending_destruction.push(std::mem::replace(&mut self.buffer, new_buffer));

        self.print_buffer_memory_usage();
    }

    fn find_place(&self, needed: usize) -> Option<usize> {
        println!("Finding place for {} bytes.", needed);
        self.gaps.iter().position(|x| x.length >= needed)
    }

    fn get_data_next_gap(&self, entry: &MeshEntry) -> Option<usize> {
        println!(
            "Getting data next gap for Mesh(id: {}, pos: {}, len: {}).",
            entry.id, entry.position, entry.length
        );
        let position = entry.position + entry.length;
        self.gaps.iter().position(|gap| gap.position == position)
    }

    fn write_at(&mut self, position: usize, data: &[u8], mesh_id: MeshId) {
        println!("Writing at {} data of len {} and of Mesh(id: {})", position, data.len(), mesh_id);
        let arena_offset = self.arena.len();
        self.arena.extend_from_slice(data);
        self.write_operations.push(WriteOperation {
            mesh_id,
            offset: position,
            len: data.len(),
            arena_offset: arena_offset,
        });
        self.schedule_batch = true;
    }

    fn batch_commands(&mut self) {
        let base_length = self.write_operations.len();

        let mut batched: Vec<WriteOperation> = Vec::with_capacity(self.write_operations.len());

        for op in self.write_operations.drain(..) {
            if let Some(last) = batched.last_mut() {
                if last.arena_offset + last.len == op.arena_offset && last.offset + last.len == op.offset {
                    last.len += op.len;
                    continue;
                }
            }
            batched.push(op);
        }

        self.write_operations = batched;

        let new_length = self.write_operations.len();
        let diff = base_length - new_length;
        if diff > 0 {
            println!("Successfully batched {} gpu commands!", diff);
        } else {
            println!("Nothing to batch.");
        }
        println!("Commands count: {}.", new_length);
    }

    pub fn flush(&mut self, queue: &Queue) {
        if self.write_operations.is_empty() {
            return;
        }
        println!("Flushing!");

        if self.schedule_batch {
            self.batch_commands();
            self.schedule_batch = false;
        }

        for op in self.write_operations.drain(..) {
            queue.write_buffer(
                self.buffer.buffer(),
                op.offset as u64,
                &self.arena[op.arena_offset..op.arena_offset + op.len],
            );
        }

        self.arena.clear();
    }

    fn insert_gap_after(&mut self, gap: Gap, position: usize) {
        println!("Inserting Gap(pos: {}, len: {}) after pos: {}.", gap.position, gap.length, position);
        let gap_index = self.gaps.iter().position(|x| x.position > position).unwrap_or(self.gaps.len());

        self.gaps.insert(gap_index, gap);
    }

    pub fn process_pending_destructions(&mut self) {
        if self.pending_destruction.is_empty() {
            return;
        }
        println!("Process pending destructions.");
        for mut buf in self.pending_destruction.drain(..) {
            buf.destroy();
        }
    }

    fn consume_gap_for(&mut self, gap_index: usize, entry: &DataEntry) {
        println!(
            "Consume Gap(index: {}) for DataEntry(id: {}, len: {}).",
            gap_index,
            entry.id,
            entry.data.len()
        );
        let position = self.gaps[gap_index].position;
        let data_length = entry.data.len();

        self.write_at(position, entry.data, entry.id);

        let gap = &mut self.gaps[gap_index];
        gap.position += data_length;
        gap.length -= data_length;

        if gap.length == 0 {
            self.gaps.remove(gap_index);
        }
    }

    pub fn update_data(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        entry: DataEntry,
    ) -> Result<(), AllocError> {
        println!("Updating data for DataEntry(id: {}, len: {}).", entry.id, entry.data.len());
        let Some(index) = self.data.iter().position(|x| x.id == entry.id) else {
            return Err(AllocError::InvalidId);
        };

        let (position, old_len) = {
            let entry = &self.data[index];
            (entry.position, entry.length)
        };
        let new_len = entry.data.len();

        // Cas 1: les nouvelles données ont une taille inférieure ou égale aux précédentes
        if new_len <= old_len {
            self.write_at(position, entry.data, entry.id);

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

            return Ok(());
        }

        // Cas 2: on regarde si la taille des anciennes données + le trou qui les suit est suffisant pour accueillir les nouvelles données
        let gap_index = self.gaps.iter().position(|x| x.position == position + old_len);

        // S'il y a un trou après
        if let Some(gap_index) = gap_index {
            let gap_length = self.gaps[gap_index].length;

            // Si taille(anciennes données + trou) suffisent
            if new_len <= old_len + gap_length {
                self.write_at(position, entry.data, entry.id);

                let gap = &mut self.gaps[gap_index];
                gap.position = position + new_len;
                gap.length = old_len + gap.length - new_len;

                if gap.length == 0 {
                    self.gaps.remove(gap_index);
                }

                self.data[index].length = new_len;

                return Ok(());
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
            self.consume_gap_for(gap_index, &entry);
            return Ok(());
        }

        // Cas 4: c'est la merde, donc on réalloue et défragmente pour avoir assez d'espace pour les nouvelles données (DERNIER RECOURS)
        let needed = self.data.iter().fold(0, |acc, x| acc + x.length) + new_len;
        self.reallocate_defragment(device, queue, encoder, needed);

        let Some(gap_index) = self.find_place(new_len) else {
            return Err(AllocError::NotEnoughSpace);
        };
        self.consume_gap_for(gap_index, &entry);

        self.print_buffer_memory_usage();

        Ok(())
    }

    fn get_new_id(&mut self) -> MeshId {
        self.free_ids.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        })
    }

    pub fn add_data(&mut self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder, data: &[u8]) -> Result<MeshId, AllocError> {
        println!("Adding data for data of len: {}.", data.len());
        let mut index = self.find_place(data.len());

        if index.is_none() {
            let needed = self.total_data_length() + data.len();
            self.reallocate_defragment(device, queue, encoder, needed);

            index = self.find_place(data.len());
        }

        let Some(index) = index else {
            return Err(AllocError::NotEnoughSpace);
        };

        let (position, gap_length) = {
            let gap = &mut self.gaps[index];

            let pos = gap.position;
            let len = gap.length;

            gap.position += data.len();
            gap.length -= data.len();

            (pos, len)
        };

        if gap_length == 0 {
            self.gaps.remove(index);
        }

        let id = self.get_new_id();

        self.write_at(position, data, id);

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

        self.print_buffer_memory_usage();

        Ok(id)
    }

    fn try_merge_gap(&mut self, position: usize) {
        println!("Trying to merge gap of pos: {}.", position);
        let Some(mut current_index) = self.gaps.iter().position(|x| x.position == position) else {
            return;
        };

        if current_index > 0 {
            let prev = &self.gaps[current_index - 1];
            let curr = &self.gaps[current_index];
            if prev.position + prev.length == curr.position {
                self.gaps[current_index - 1].length += curr.length;
                self.gaps.remove(current_index);
                current_index -= 1;
            }
        }

        let current = &self.gaps[current_index];
        let curr_end = current.position + current.length;

        if current_index + 1 < self.gaps.len() {
            let next = &self.gaps[current_index + 1];
            if next.position == curr_end {
                self.gaps[current_index].length += next.length;
                self.gaps.remove(current_index + 1);
            }
        }
    }

    pub fn free_data(&mut self, id: MeshId) -> Result<(), AllocError> {
        println!("Freeing data of mesh id: {}.", id);
        let Some(data_index) = self.data.iter().position(|x| x.id == id) else {
            return Err(AllocError::InvalidId);
        };

        let entry = &self.data[data_index];
        let (position, length) = (entry.position, entry.length);

        let gap = Gap::new(position, length);

        self.insert_gap_after(gap, position);
        self.try_merge_gap(position);

        self.data.remove(data_index);
        self.write_operations.retain(|element| element.mesh_id != id);

        self.free_ids.push(id);

        self.print_buffer_memory_usage();

        Ok(())
    }
}
