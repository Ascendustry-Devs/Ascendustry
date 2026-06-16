use std::{
    mem::replace,
    sync::{Arc, RwLock},
};

use project_core::utils::id_pool::IdPool;
use rustc_hash::FxHashSet;
use wgpu::{Buffer, BufferAddress, BufferUsages, CommandEncoder};

use crate::{
    gpu::{
        allocator::{
            entry::{AllocEntry, EntryId},
            error::AllocError,
            gap_manager::{Gap, GapManager},
            write_operation::WriteOperation,
            ARENA_MIN_SIZE, BUFFER_BASE_SIZE, BUFFER_EXPAND_COEF, LOG_ALLOCATOR,
        },
        smart_buffer::SmartBuffer,
        tools::GpuTools,
    },
    log_allocator,
};

pub struct GpuAllocator {
    pub data: Vec<AllocEntry>,

    id_pool: IdPool,

    // GapManager
    gaps: Vec<Gap>,

    // Interface
    pending_destruction: Vec<SmartBuffer>,
    write_operations: Vec<WriteOperation>,
    schedule_batch: bool,
    arena: Vec<u8>,

    // GPU
    buffer: SmartBuffer,
    gpu_tools: Arc<GpuTools>,
    frame_encoder: Arc<RwLock<CommandEncoder>>,
}

impl GapManager for GpuAllocator {
    fn gaps(&self) -> &Vec<Gap> {
        &self.gaps
    }

    fn gaps_mut(&mut self) -> &mut Vec<Gap> {
        &mut self.gaps
    }
}

impl GpuAllocator {
    pub fn new(gpu_tools: Arc<GpuTools>, frame_encoder: Arc<RwLock<CommandEncoder>>) -> Self {
        let buffer = SmartBuffer::from_capacity(
            BUFFER_BASE_SIZE as u32,
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
            id_pool: IdPool::new(),
            write_operations: vec![],
            schedule_batch: false,
            arena: Vec::with_capacity(ARENA_MIN_SIZE),
        }
    }

    pub fn get_entry_index(&self, id: EntryId) -> Result<usize, AllocError> {
        match self.data.iter().position(|x| x.id == id) {
            Some(index) => Ok(index),
            None => Err(AllocError::InvalidId),
        }
    }

    pub fn get_entry(&self, id: EntryId) -> Result<&AllocEntry, AllocError> {
        let index = self.get_entry_index(id)?;
        Ok(&self.data[index])
    }

    pub fn iter_entries_by_intersection<'a>(&'a self, ids: &'a FxHashSet<EntryId>) -> impl Iterator<Item = &'a AllocEntry> {
        self.data.iter().filter(move |e| ids.contains(&e.id))
    }

    fn get_entries_length(&self) -> usize {
        self.data.iter().fold(0, |acc, x| acc + x.length)
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
            + self.id_pool.len() * size_of::<u32>()
            + self.gaps.len() * size_of::<Gap>()
            + self.pending_destruction.len() * size_of::<SmartBuffer>()
            + self.write_operations.len() * size_of::<WriteOperation>()
            + self.data.len() * size_of::<AllocEntry>()) as u32;
        let (used_cpu_mb, used_cpu_kb, used_cpu_b) = conversion(used_cpu);

        let alloc_cpu = (self.arena.capacity()
            + self.id_pool.capacity() * size_of::<u32>() // Ceci est un easter egg: https://pastebin.com/demZGt0P
            + self.gaps.capacity() * size_of::<Gap>()
            + self.pending_destruction.capacity() * size_of::<SmartBuffer>()
            + self.write_operations.capacity() * size_of::<WriteOperation>()
            + self.data.capacity() * size_of::<AllocEntry>()) as u32;
        let (alloc_cpu_mb, alloc_cpu_kb, alloc_cpu_b) = conversion(alloc_cpu);

        let data_length = self.get_entries_length() as u32;
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
        let mut position = self.find_place(data.len());

        // If we could not find enough space,
        // we need to reallocate to get more.
        if position.is_none() {
            let needed = self.get_entries_length() + data.len();
            self.reallocate_defragment(needed);

            position = self.find_place(data.len());
        }

        // If we STILL can not find enough space,
        // it means that we reached the maximum capacity.
        let Some(gap_index) = position else {
            return Err(AllocError::NotEnoughSpace);
        };

        let id = self.insert_data_at_gap(None, data, gap_index);

        log_allocator!("Added entry for data of len {}.", data.len());
        self.print_debug_infos();

        Ok(id)
    }

    pub fn update(&mut self, id: u32, data: &[u8]) -> Result<(), AllocError> {
        log_allocator!("Updating data for DataEntry(id: {}, len: {}).", id, data.len());
        let index = self.get_entry_index(id)?;

        let (position, old_len) = {
            let entry = &self.data[index];
            (entry.position, entry.length)
        };
        let new_len = data.len();

        // Cas 1: les nouvelles données ont une taille inférieure ou égale aux précédentes
        if new_len <= old_len {
            if new_len < old_len {
                self.release_after(position + new_len, old_len - new_len);
            }

            self.data[index].length = new_len;
            self.write_at(position, data, id);

            return Ok(());
        }

        // Cas 2: on regarde si la taille des anciennes données + le trou qui les suit est suffisant pour accueillir les nouvelles données
        let gap_index = self.get_gap_index_at(position + old_len).ok();

        // S'il y a un trou après
        if let Some(gap_index) = gap_index {
            let gap = &mut self.gaps[gap_index];
            let available_space = old_len + gap.length; // anciennes données + trou

            // Si taille suffisent
            if new_len <= available_space {
                let delta_len = new_len - old_len;
                self.consume_gap(gap_index, delta_len);

                self.data[index].length = new_len;
                // self.try_merge_gap(self.gaps[gap_index].position);
                self.write_at(position, data, id);

                return Ok(());
            }
            // Si ça suffit pas, on élargit le trou à (ancienne données + trou actuel) et donc on marque l'emplacement comme libre
            gap.extend_left(old_len);
        }
        // S'il n'y a pas de trou après les données actuelle, on a pas la place pour stocker les nouvelles. On marque alors l'emplacement actuel comme libre
        else {
            let gap = Gap::new(position, old_len);
            self.insert_gap_at(gap, position);
        }

        // On ne peut plus garder l'emplacement actuel pour cette entrée.
        // On doit la supprimer entièrement pour l'instant.
        self.delete_entry(index, id);

        // Cas 3: on regarde s'il existe un trou suffisant pour accueillir les nouvelles données...
        if let Some(gap_index) = self.find_place(new_len) {
            self.insert_data_at_gap(Some(id), data, gap_index);
            return Ok(());
        }

        // Cas 4: c'est la merde, donc on réalloue et défragmente pour avoir assez d'espace pour les nouvelles données (DERNIER RECOURS)
        let needed = self.get_entries_length() + new_len;
        self.reallocate_defragment(needed);

        // If after all these efforts we STILL can't find enough space,
        // it means that we reached the maximum capacity.
        let Some(gap_index) = self.find_place(new_len) else {
            return Err(AllocError::NotEnoughSpace);
        };

        self.insert_data_at_gap(Some(id), data, gap_index);

        self.print_debug_infos();

        Ok(())
    }

    pub fn free(&mut self, id: EntryId) -> Result<(), AllocError> {
        log_allocator!("Freeing data of mesh id: {}.", id);
        let entry_index = self.get_entry_index(id)?;

        let (position, length) = {
            let entry = &self.data[entry_index];
            (entry.position, entry.length)
        };

        self.delete_entry(entry_index, id);
        self.release_after(position, length);

        self.id_pool.free_id(id);

        self.print_debug_infos();

        Ok(())
    }

    pub fn flush(&mut self) {
        if self.write_operations.is_empty() {
            return;
        }

        if self.schedule_batch {
            self.batch_commands();
            self.schedule_batch = false;
        }

        let buffer = self.buffer.buffer();
        let arena = &self.arena;

        for op in self.write_operations.drain(..) {
            self.gpu_tools.queue().write_buffer(
                buffer,
                op.buffer_offset as u64,
                &arena[op.arena_offset..op.arena_offset + op.len],
            );
        }

        self.arena.clear();

        log_allocator!("Flushed!");
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

    fn insert_entry(&mut self, entry: AllocEntry) {
        let index = match self.data.binary_search_by_key(&entry.position, |e| e.position) {
            Ok(i) => i,
            Err(i) => i,
        };
        self.data.insert(index, entry);
    }

    /// Returns `Some(id)` if no id was provided.
    fn insert_data_at_gap(&mut self, id: Option<EntryId>, data: &[u8], gap_index: usize) -> EntryId {
        let data_length = data.len();
        let id = match id {
            Some(id) => id,
            None => self.id_pool.get_new_id(),
        };

        let position = self.consume_gap(gap_index, data_length);
        let entry = AllocEntry::new(id, position, data_length);
        self.insert_entry(entry);
        self.write_at(position, data, id);

        id
    }

    fn delete_entry(&mut self, entry_index: usize, entry_id: EntryId) {
        self.data.remove(entry_index);
        self.write_operations.retain(|operation| operation.id != entry_id);
    }

    pub fn reallocate_defragment(&mut self, needed: usize) {
        log_allocator!(
            "Reallocate and defragment because current buffer has {} bytes of capacity but we need {} bytes.",
            self.get_entries_length(),
            needed
        );

        if !self.write_operations.is_empty() {
            self.flush();
        }

        let device = self.gpu_tools.device();

        let old_smart_buffer = &self.buffer;
        let old_buffer = old_smart_buffer.buffer();

        // Reallocate
        let needed = (needed as f32 * BUFFER_EXPAND_COEF) as u32;
        let new_smart_buffer = SmartBuffer::from_capacity(needed, device, old_smart_buffer.format(), old_smart_buffer.usages());
        let new_buffer = new_smart_buffer.buffer();

        let mut current_position = 0;
        let mut encoder = self.frame_encoder.write().unwrap();

        // Copy each entry to the new buffer without any gaps
        for entry in &mut self.data {
            encoder.copy_buffer_to_buffer(
                old_buffer,
                entry.position as BufferAddress,
                new_buffer,
                current_position as BufferAddress,
                entry.length as BufferAddress,
            );

            // Update entry
            entry.position = current_position;

            current_position += entry.length;
        }

        // Update gaps
        self.gaps.clear();
        let gap_length = new_smart_buffer.capacity() as usize - current_position;
        if gap_length != 0 {
            let gap = Gap::new(current_position, gap_length);
            self.gaps.push(gap);
        }

        // Update buffer with the newer and destroy the older one
        self.pending_destruction.push(replace(&mut self.buffer, new_smart_buffer));

        self.print_debug_infos();
    }

    fn write_at(&mut self, buffer_offset: usize, data: &[u8], mesh_id: EntryId) {
        log_allocator!(
            "Writing at {} data of len {} and of Mesh(id: {})",
            buffer_offset,
            data.len(),
            mesh_id
        );

        let len = data.len();
        let arena_offset = self.arena.len();

        self.arena.extend_from_slice(data);

        let operation = WriteOperation::new(mesh_id, len, buffer_offset, arena_offset);
        self.write_operations.push(operation);

        self.schedule_batch = true;
    }

    fn batch_commands(&mut self) {
        let base_len = self.write_operations.len();
        if base_len <= 1 {
            return;
        }

        let mut head = 0;

        for i in 1..base_len {
            let can_merge = {
                let curr = &self.write_operations[head];
                let next = &self.write_operations[i];
                curr.arena_offset + curr.len == next.arena_offset && curr.buffer_offset + curr.len == next.buffer_offset
            };
            if can_merge {
                let consumed_len = self.write_operations[i].len;
                self.write_operations[head].len += consumed_len;
            } else {
                head += 1;
                if head != i {
                    self.write_operations.swap(head, i);
                }
            }
        }

        self.write_operations.truncate(head + 1);
    }
}
