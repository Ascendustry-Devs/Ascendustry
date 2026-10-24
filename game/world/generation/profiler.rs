use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Clone, Debug, Default)]
pub struct GenTimeStats {
    pub count: u64,
    pub sum_nanos: u128,
    pub min_nanos: u128,
    pub max_nanos: u128,
    pub last_nanos: u128,
}

pub const GEN_HIST_LEN: usize = 4096;

pub static mut GEN_HIST_TOTAL: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_NOISE: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_SURFACE: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_TERRAIN: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_CAVE: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_ORE: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_VEGETATION: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_OTHER: [u128; GEN_HIST_LEN] = [0u128; GEN_HIST_LEN];
pub static mut GEN_HIST_IDX: usize = 0;

pub static mut GEN_STAT_TOTAL: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_NOISE: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_SURFACE: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_TERRAIN: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_CAVE: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_ORE: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_VEGETATION: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};
pub static mut GEN_STAT_OTHER: GenTimeStats = GenTimeStats {
    count: 0,
    sum_nanos: 0,
    min_nanos: u128::MAX,
    max_nanos: 0,
    last_nanos: 0,
};

pub static TIME_SPENT_GENERATING_NANOS: AtomicU64 = AtomicU64::new(0);
pub static CHUNKS_GENERATED: AtomicU32 = AtomicU32::new(0);

#[inline]
pub fn gen_record_total(nanos: u128) {
    unsafe {
        let idx = GEN_HIST_IDX % GEN_HIST_LEN;
        GEN_HIST_TOTAL[idx] = nanos;
        GEN_HIST_IDX = (GEN_HIST_IDX + 1) % GEN_HIST_LEN;

        let s = &mut GEN_STAT_TOTAL;
        s.count += 1;
        s.sum_nanos += nanos;
        if nanos < s.min_nanos {
            s.min_nanos = nanos;
        }
        if nanos > s.max_nanos {
            s.max_nanos = nanos;
        }
        s.last_nanos = nanos;
    }
    let _ = TIME_SPENT_GENERATING_NANOS.fetch_add(nanos.min(u128::from(u64::MAX)) as u64, Ordering::Relaxed);
    let _ = nanos; // keep for future use
    CHUNKS_GENERATED.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub fn gen_record_section(stat: &mut GenTimeStats, hist: &mut [u128; GEN_HIST_LEN], nanos: u128, idx_base: &mut usize) {
    let idx = *idx_base % GEN_HIST_LEN;
    hist[idx] = nanos;
    *idx_base = (*idx_base + 1) % GEN_HIST_LEN;

    stat.count += 1;
    stat.sum_nanos += nanos;
    if nanos < stat.min_nanos {
        stat.min_nanos = nanos;
    }
    if nanos > stat.max_nanos {
        stat.max_nanos = nanos;
    }
    stat.last_nanos = nanos;
}
