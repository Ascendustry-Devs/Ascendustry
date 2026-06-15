use std::{
    mem,
    sync::{Arc, RwLock},
};

use wgpu::{Buffer, BufferUsages, CommandEncoder};

use crate::gpu::{
    allocator::data_structures::{AllocError, Gap, WriteOperation},
    smart_buffer::SmartBuffer,
    tools::GpuTools,
};

// const BYTES_PER_FRAME_CAP: usize = 1024 * 1024 * 8;
// const MAX_MILLIS_PER_FRAME_CAP: u128 = 8;
// const MAX_WRITE_OPERATIONS_PER_FRAME: usize = 5;
const ARENA_MIN_SIZE: usize = 1024 * 1024 * 4; // 4Mb
const MESH_BUFFER_BASE_SIZE: usize = 1024 * 1024 * 4; // 4Mb
const MESH_BUFFER_EXPAND_COEF: f32 = 1.25; // allocates 1.25x more than needed to prevent quick reallocates.

pub type EntryId = u32;

#[derive(Debug, PartialEq, Eq)]
pub struct AllocEntry {
    pub id: EntryId,
    pub position: usize,
    pub length: usize,
}

impl AllocEntry {
    pub fn new(id: EntryId, position: usize, length: usize) -> Self {
        Self { id, position, length }
    }
}

trait IdManager {
    fn next_id(&mut self) -> &mut u32;
    fn free_ids(&mut self) -> &mut Vec<u32>;

    fn get_new_id(&mut self) -> u32 {
        self.free_ids().pop().unwrap_or_else(|| {
            let id = *self.next_id();
            *self.next_id() += 1;
            id
        })
    }

    fn free_id(&mut self, id: u32) {
        self.free_ids().push(id);
    }
}

pub struct GpuAllocator {
    pub data: Vec<AllocEntry>,
    next_id: EntryId,
    free_ids: Vec<EntryId>,

    gaps: Vec<Gap>,
    pending_destruction: Vec<SmartBuffer>,
    write_operations: Vec<WriteOperation>,
    schedule_batch: bool,
    arena: Vec<u8>,

    // GPU
    buffer: SmartBuffer,
    gpu_tools: Arc<GpuTools>,
    frame_encoder: Arc<RwLock<CommandEncoder>>,
}

impl IdManager for GpuAllocator {
    fn next_id(&mut self) -> &mut u32 {
        &mut self.next_id
    }

    fn free_ids(&mut self) -> &mut Vec<u32> {
        &mut self.free_ids
    }
}

const LOG_ALLOCATOR: bool = false;

macro_rules! log_allocator {
    () => {
        if LOG_ALLOCATOR {
            println!();
        }
    };

    ($($arg:tt)*) => {
        if LOG_ALLOCATOR {
            println!($($arg)*);
        }
    };
}

impl GpuAllocator {
    pub fn new(gpu_tools: Arc<GpuTools>, frame_encoder: Arc<RwLock<CommandEncoder>>) -> Self {
        let buffer = SmartBuffer::from_capacity(
            MESH_BUFFER_BASE_SIZE as u32,
            gpu_tools.device(),
            None,
            BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX,
        );

        Self {
            gpu_tools,
            frame_encoder,
            buffer,
            data: Vec::with_capacity(256),
            gaps: Vec::with_capacity(32),
            pending_destruction: Vec::with_capacity(1),
            next_id: 0,
            free_ids: Vec::with_capacity(8),
            write_operations: vec![],
            schedule_batch: false,
            arena: Vec::with_capacity(ARENA_MIN_SIZE),
        }
    }

    pub fn get_mesh_entry(&self, id: EntryId) -> Option<&AllocEntry> {
        if let Some(i) = self.data.iter().position(|entry| entry.id == id) {
            return self.data.get(i);
        }
        None
    }

    pub fn get_entries_difference_by_ids(&self, ids: Vec<EntryId>) -> Vec<&AllocEntry> {
        self.data.iter().filter(|entry| !ids.contains(&entry.id)).collect()
    }

    pub fn print_debug_infos(&self) {
        if LOG_ALLOCATOR {
            self.force_print_debug_infos();
        }
    }

    pub fn force_print_debug_infos(&self) {
        let conversion = |b: u32| {
            let kb = b / 1024;
            let mb = kb / 1024;
            return (mb, kb, b);
        };

        let mesh_count = self.data.len();

        let used_cpu = (self.arena.len()
            + self.free_ids.len() * size_of::<u32>()
            + self.gaps.len() * size_of::<Gap>()
            + self.pending_destruction.len() * size_of::<SmartBuffer>()
            + self.write_operations.len() * size_of::<WriteOperation>()
            + self.data.len() * size_of::<AllocEntry>()) as u32;
        let (used_cpu_mb, used_cpu_kb, used_cpu_b) = conversion(used_cpu);

        let alloc_cpu = (self.arena.capacity()
            + self.free_ids.capacity() * size_of::<u32>() // Ceci est un easter egg: https://pastebin.com/demZGt0P
            + self.gaps.capacity() * size_of::<Gap>()
            + self.pending_destruction.capacity() * size_of::<SmartBuffer>()
            + self.write_operations.capacity() * size_of::<WriteOperation>()
            + self.data.capacity() * size_of::<AllocEntry>()) as u32;
        let (alloc_cpu_mb, alloc_cpu_kb, alloc_cpu_b) = conversion(alloc_cpu);

        let data_length = self.total_data_length() as u32;
        let (data_mb, data_kb, data_b) = conversion(data_length);

        let alloc_gpu = self.buffer.capacity();
        let (alloc_gpu_mb, alloc_gpu_kb, alloc_gpu_b) = conversion(alloc_gpu);

        println!("Mesh count: {}", mesh_count);
        println!(
            "Allocated Memory (CPU) {:3}Mb | {:6}Kb | {:9}b",
            alloc_cpu_mb, alloc_cpu_kb, alloc_cpu_b
        );
        println!(
            "└─ Free                {:3}Mb | {:6}Kb | {:9}b",
            alloc_cpu_mb - used_cpu_mb,
            alloc_cpu_kb - used_cpu_kb,
            alloc_cpu_b - used_cpu_b
        );
        println!(
            "└─ Used                {:3}Mb | {:6}Kb | {:9}b",
            used_cpu_mb, used_cpu_kb, used_cpu_b
        );
        println!(
            "Allocated Memory (GPU) {:3}Mb | {:6}Kb | {:9}b",
            alloc_gpu_mb, alloc_gpu_kb, alloc_gpu_b
        );
        println!(
            "└─ Free                {:3}Mb | {:6}Kb | {:9}b",
            alloc_gpu_mb - data_mb,
            alloc_gpu_kb - data_kb,
            alloc_gpu_b - data_b
        );
        println!("└─ Used                {:3}Mb | {:6}Kb | {:9}b", data_mb, data_kb, data_b);
    }

    pub fn get_buffer(&self) -> &Buffer {
        self.buffer.buffer()
    }

    pub fn add(&mut self, data: &[u8]) -> Result<EntryId, AllocError> {
        log_allocator!("Adding data for data of len: {}.", data.len());
        let mut index = self.find_place(data.len());

        if index.is_none() {
            let needed = self.total_data_length() + data.len();
            self.reallocate_defragment(needed);

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

        let entry = AllocEntry::new(id, position, data.len());
        self.insert_entry(entry);

        self.print_debug_infos();

        Ok(id)
    }

    pub fn update(&mut self, id: u32, data: &[u8]) -> Result<(), AllocError> {
        log_allocator!("Updating data for DataEntry(id: {}, len: {}).", id, data.len());
        let Some(index) = self.data.iter().position(|x| x.id == id) else {
            return Err(AllocError::InvalidId);
        };

        let (position, old_len) = {
            let entry = &self.data[index];
            (entry.position, entry.length)
        };
        let new_len = data.len();

        // Cas 1: les nouvelles données ont une taille inférieure ou égale aux précédentes
        if new_len <= old_len {
            self.write_at(position, data, id);

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
                self.write_at(position, data, id);

                let gap = &mut self.gaps[gap_index];
                gap.position = position + new_len;
                gap.length = old_len + gap.length - new_len;

                if gap.length == 0 {
                    self.gaps.remove(gap_index);
                }

                self.data[index].length = new_len;

                self.try_merge_gap(self.gaps[gap_index].position);

                return Ok(());
            }
            // Si ça suffit pas, on élargit le trou à (ancienne données + trou actuel) et donc on marque l'emplacement comme libre
            else {
                let gap = &mut self.gaps[gap_index];
                gap.position = position;
                gap.length += old_len;

                self.data.remove(index);
                self.write_operations.retain(|e| e.mesh_id != id);
            }
        }
        // S'il n'y a pas de trou après les données actuelle, on a pas la place pour stocker les nouvelles. On marque alors l'emplacement actuel comme libre
        else {
            let gap = Gap::new(position, old_len);
            self.insert_gap_after(gap, position);

            self.data.remove(index);
            self.write_operations.retain(|e| e.mesh_id != id);
        }

        // Cas 3: on regarde s'il existe un trou suffisant pour accueillir les nouvelles données...
        if let Some(gap_index) = self.find_place(new_len) {
            let position = self.consume_gap_for(gap_index, id, data);
            let entry = AllocEntry::new(id, position, new_len);
            self.insert_entry(entry);

            return Ok(());
        }

        // Cas 4: c'est la merde, donc on réalloue et défragmente pour avoir assez d'espace pour les nouvelles données (DERNIER RECOURS)
        let needed = self.data.iter().fold(0, |acc, x| acc + x.length) + new_len;
        self.reallocate_defragment(needed);

        let Some(gap_index) = self.find_place(new_len) else {
            return Err(AllocError::NotEnoughSpace);
        };
        let position = self.consume_gap_for(gap_index, id, data);
        let entry = AllocEntry::new(id, position, new_len);
        self.insert_entry(entry);

        self.print_debug_infos();

        Ok(())
    }

    pub fn free(&mut self, id: EntryId) -> Result<(), AllocError> {
        log_allocator!("Freeing data of mesh id: {}.", id);
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

        self.free_id(id);

        self.print_debug_infos();

        Ok(())
    }

    pub fn flush(&mut self) {
        if self.write_operations.is_empty() {
            return;
        }
        log_allocator!("Flushing!");

        if self.schedule_batch {
            self.batch_commands();
            self.schedule_batch = false;
        }

        for op in self.write_operations.drain(..) {
            self.gpu_tools.queue().write_buffer(
                self.buffer.buffer(),
                op.offset as u64,
                &self.arena[op.arena_offset..op.arena_offset + op.len],
            );
        }

        self.arena.clear();
    }

    pub fn process_pending_destructions(&mut self) {
        if self.pending_destruction.is_empty() {
            return;
        }
        log_allocator!("Process pending destructions.");
        for mut buf in self.pending_destruction.drain(..) {
            buf.destroy();
        }
    }

    fn find_place(&self, needed: usize) -> Option<usize> {
        log_allocator!("Finding place for {} bytes.", needed);
        self.gaps.iter().position(|x| x.length >= needed)
    }

    fn insert_entry(&mut self, entry: AllocEntry) {
        let entry_index = self
            .data
            .iter()
            .position(|x| x.position > entry.position)
            .unwrap_or(self.data.len());
        self.data.insert(entry_index, entry);
    }

    fn get_data_next_gap(&self, entry: &AllocEntry) -> Option<usize> {
        log_allocator!(
            "Getting data next gap for Mesh(id: {}, pos: {}, len: {}).",
            entry.id,
            entry.position,
            entry.length
        );
        let position = entry.position + entry.length;
        // self.gaps.iter().position(|gap| gap.position == position)
        self.gaps.binary_search_by_key(&position, |gap| gap.position).ok()
    }

    fn total_data_length(&self) -> usize {
        self.data.iter().fold(0, |acc, v| acc + v.length)
    }

    pub fn reallocate_defragment(&mut self, needed: usize) {
        log_allocator!(
            "Reallocate and defragment because current buffer has {} bytes of capacity but we need {} bytes.",
            self.total_data_length(),
            needed
        );
        let needed = (needed as f32 * MESH_BUFFER_EXPAND_COEF) as u32;

        if !self.write_operations.is_empty() {
            self.flush();
        }

        let device = self.gpu_tools.device();

        // Reallocate
        let new_buffer = SmartBuffer::from_capacity(
            needed,
            device,
            None,
            BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::VERTEX,
        );

        let mut current_position = 0;

        // Copy each entry to the new buffer without any gaps
        for entry in &mut self.data {
            self.frame_encoder.write().unwrap().copy_buffer_to_buffer(
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
        let gap_length = new_buffer.capacity() as usize - current_position;
        if gap_length != 0 {
            let gap = Gap::new(current_position, gap_length);
            self.gaps.push(gap);
        }

        // Update buffer
        self.pending_destruction.push(mem::replace(&mut self.buffer, new_buffer));

        self.print_debug_infos();
    }

    fn write_at(&mut self, offset: usize, data: &[u8], mesh_id: EntryId) {
        log_allocator!(
            "Writing at {} data of len {} and of Mesh(id: {})",
            offset,
            data.len(),
            mesh_id
        );

        let len = data.len();
        let arena_offset = self.arena.len();

        self.arena.extend_from_slice(data);

        let operation = WriteOperation::new(mesh_id, offset, len, arena_offset);
        self.write_operations.push(operation);

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
            log_allocator!("Successfully batched {} gpu commands!", diff);
        } else {
            log_allocator!("Nothing to batch.");
        }
        log_allocator!("Commands count: {}.", new_length);
    }

    fn insert_gap_after(&mut self, new_gap: Gap, position: usize) {
        log_allocator!(
            "Inserting Gap(pos: {}, len: {}) after pos: {}.",
            new_gap.position,
            new_gap.length,
            position
        );
        let index = self
            .gaps
            .iter()
            .position(|gap| gap.position > new_gap.position)
            .unwrap_or(self.gaps.len());

        self.gaps.insert(index, new_gap);
    }

    fn consume_gap_for(&mut self, gap_index: usize, id: u32, data: &[u8]) -> usize {
        log_allocator!(
            "Consume Gap(index: {}) for DataEntry(id: {}, len: {}).",
            gap_index,
            id,
            data.len()
        );
        let gap_pos = self.gaps[gap_index].position;
        let data_length = data.len();

        self.write_at(gap_pos, data, id);

        let gap = &mut self.gaps[gap_index];
        gap.position += data_length;
        gap.length -= data_length;

        if gap.length == 0 {
            self.gaps.remove(gap_index);
        }

        gap_pos
    }

    fn try_merge_gap(&mut self, position: usize) {
        log_allocator!("Trying to merge gap of pos: {}.", position);
        let Some(mut current_index) = self.gaps.iter().position(|x| x.position == position) else {
            return;
        };

        self.try_merge_prev_gap(&mut current_index);
        self.try_merge_next_gap(current_index);
    }

    fn try_merge_prev_gap(&mut self, current_index: &mut usize) {
        let idx = *current_index;
        if idx > 0 {
            let prev = &self.gaps[idx - 1];
            let curr = &self.gaps[idx];
            if prev.position + prev.length == curr.position {
                self.gaps[idx - 1].length += curr.length;
                self.gaps.remove(idx);
                *current_index -= 1;
            }
        }
    }

    fn try_merge_next_gap(&mut self, current_index: usize) {
        if current_index + 1 < self.gaps.len() {
            let curr = &self.gaps[current_index];
            let next = &self.gaps[current_index + 1];
            if curr.position + curr.length == next.position {
                self.gaps[current_index].length += next.length;
                self.gaps.remove(current_index + 1);
            }
        }
    }
}
