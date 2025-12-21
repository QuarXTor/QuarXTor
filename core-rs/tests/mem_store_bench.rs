use std::collections::HashMap;
use std::hint::black_box;
use std::time::{Duration, Instant};

use quarxtor_core::block::multi::MultiRecipe;
use quarxtor_core::codec::{ObjectPayload, ZPayload};
use quarxtor_core::store::blockstore::{
    decode_frame_typed, make_frame_l0, make_frame_multi, make_frame_object, make_frame_z,
    BlockStore, StoreError, StoreResult,
};
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::store::mem_store::MemBlockStore;
use quarxtor_core::types::{BlockId, BlockKind};

const BLOCK_SIZE: usize = 64 * 1024;
const BLOCKS: usize = 1024;
const SEED: u64 = 0x9E3779B97F4A7C15;

struct MemStore {
    frames: HashMap<BlockId, Vec<u8>>,
    next_id: BlockId,
}

impl MemStore {
    fn new() -> Self {
        Self {
            frames: HashMap::new(),
            next_id: 0,
        }
    }

    fn alloc_id(&mut self) -> BlockId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

impl BlockStore for MemStore {
    fn put_l0(&mut self, raw: &[u8]) -> StoreResult<BlockId> {
        let id = self.alloc_id();
        let frame = make_frame_l0(id, raw);
        self.frames.insert(id, frame);
        Ok(id)
    }

    fn put_multi(&mut self, recipe: &MultiRecipe) -> StoreResult<BlockId> {
        let id = self.alloc_id();
        let frame = make_frame_multi(id, recipe);
        self.frames.insert(id, frame);
        Ok(id)
    }

    fn put_z(&mut self, z: &ZPayload) -> StoreResult<BlockId> {
        let id = self.alloc_id();
        let frame = make_frame_z(id, z);
        self.frames.insert(id, frame);
        Ok(id)
    }

    fn put_object(&mut self, o: &ObjectPayload) -> StoreResult<BlockId> {
        let id = self.alloc_id();
        let frame = make_frame_object(id, o);
        self.frames.insert(id, frame);
        Ok(id)
    }

    fn get_typed(&self, id: BlockId) -> StoreResult<(BlockKind, [u8; 32], BlockBody)> {
        let frame = self
            .frames
            .get(&id)
            .ok_or(StoreError::OutOfRange(id))?;
        let (kind, decoded_id, hash, body) = decode_frame_typed(frame)?;
        if decoded_id != id {
            return Err(StoreError::Corrupt(format!(
                "id mismatch: requested {}, frame {}",
                id, decoded_id
            )));
        }
        Ok((kind, hash, body))
    }

    fn get_frame(&self, id: BlockId) -> StoreResult<Vec<u8>> {
        let frame = self
            .frames
            .get(&id)
            .ok_or(StoreError::OutOfRange(id))?;
        Ok(frame.clone())
    }
}

fn fill_block(buf: &mut [u8], seed: u64) {
    let mut s = seed;
    for chunk in buf.chunks_mut(8) {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        s = s.wrapping_mul(0x2545F4914F6CDD1D);
        let bytes = s.to_le_bytes();
        let len = chunk.len();
        chunk.copy_from_slice(&bytes[..len]);
    }
}

fn read_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse::<u64>().ok();
        }
    }
    None
}

fn mib_per_sec(bytes: u64, dur: Duration) -> f64 {
    let secs = dur.as_secs_f64();
    if secs == 0.0 {
        0.0
    } else {
        (bytes as f64) / (1024.0 * 1024.0) / secs
    }
}

fn fmt_opt_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "na".to_string())
}

#[test]
#[ignore]
fn mem_store_baseline_bench() {
    let blocks = BLOCKS;
    let block_size = BLOCK_SIZE;
    let total_bytes = (blocks as u64) * (block_size as u64);

    let mut store = MemStore::new();
    let mut payload = vec![0u8; block_size];

    let rss_before = read_rss_kb();

    let write_start = Instant::now();
    for i in 0..blocks {
        fill_block(&mut payload, SEED ^ (i as u64));
        store.put_l0(&payload).expect("put_l0");
    }
    let write_dur = write_start.elapsed();
    let rss_after_write = read_rss_kb();

    let read_start = Instant::now();
    let mut checksum: u64 = 0;
    for id in 0..(blocks as BlockId) {
        let frame = store.get_frame(id).expect("get_frame");
        checksum = checksum.wrapping_add(frame[0] as u64);
        black_box(&frame);
    }
    let read_dur = read_start.elapsed();
    let rss_after_read = read_rss_kb();

    println!(
        "mem_store_baseline: blocks={} block_size={} bytes={} seed=0x{:016x}",
        blocks, block_size, total_bytes, SEED
    );
    println!(
        "write: {:.6}s {:.2} MiB/s",
        write_dur.as_secs_f64(),
        mib_per_sec(total_bytes, write_dur)
    );
    println!(
        "read: {:.6}s {:.2} MiB/s checksum={}",
        read_dur.as_secs_f64(),
        mib_per_sec(total_bytes, read_dur),
        checksum
    );
    println!(
        "rss_kb: before={} after_write={} after_read={}",
        fmt_opt_u64(rss_before),
        fmt_opt_u64(rss_after_write),
        fmt_opt_u64(rss_after_read)
    );
}

#[test]
#[ignore]
fn mem_store_dense_bench() {
    let blocks = BLOCKS;
    let block_size = BLOCK_SIZE;
    let total_bytes = (blocks as u64) * (block_size as u64);

    let mut store = MemBlockStore::new();
    let mut payload = vec![0u8; block_size];

    let rss_before = read_rss_kb();

    let write_start = Instant::now();
    for i in 0..blocks {
        fill_block(&mut payload, SEED ^ (i as u64));
        store.put_l0(&payload).expect("put_l0");
    }
    let write_dur = write_start.elapsed();
    let rss_after_write = read_rss_kb();

    let read_start = Instant::now();
    let mut checksum: u64 = 0;
    for id in 0..(blocks as BlockId) {
        let frame = store.get_frame(id).expect("get_frame");
        checksum = checksum.wrapping_add(frame[0] as u64);
        black_box(&frame);
    }
    let read_dur = read_start.elapsed();
    let rss_after_read = read_rss_kb();

    println!(
        "mem_store_dense: blocks={} block_size={} bytes={} seed=0x{:016x}",
        blocks, block_size, total_bytes, SEED
    );
    println!(
        "write: {:.6}s {:.2} MiB/s",
        write_dur.as_secs_f64(),
        mib_per_sec(total_bytes, write_dur)
    );
    println!(
        "read: {:.6}s {:.2} MiB/s checksum={}",
        read_dur.as_secs_f64(),
        mib_per_sec(total_bytes, read_dur),
        checksum
    );
    println!(
        "rss_kb: before={} after_write={} after_read={}",
        fmt_opt_u64(rss_before),
        fmt_opt_u64(rss_after_write),
        fmt_opt_u64(rss_after_read)
    );
}
