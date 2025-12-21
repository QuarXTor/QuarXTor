use crate::block::multi::MultiRecipe;
use crate::codec::{ObjectPayload, ZPayload};
use crate::store::blockstore::{
    decode_frame_typed, make_frame_l0, make_frame_multi, make_frame_object, make_frame_z,
    BlockStore, StoreError, StoreResult,
};
use crate::store::decode::BlockBody;
use crate::types::{BlockId, BlockKind};

/// In-memory block store with dense ID space.
pub struct MemBlockStore {
    frames: Vec<Option<Vec<u8>>>,
}

impl MemBlockStore {
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    fn next_id(&self) -> BlockId {
        self.frames.len() as BlockId
    }

    fn index_of(id: BlockId) -> StoreResult<usize> {
        usize::try_from(id).map_err(|_| StoreError::OutOfRange(id))
    }

    fn frame_at(&self, id: BlockId) -> StoreResult<&[u8]> {
        let idx = Self::index_of(id)?;
        let slot = self.frames.get(idx).ok_or(StoreError::OutOfRange(id))?;
        slot.as_deref().ok_or(StoreError::OutOfRange(id))
    }

    /// Return existing BlockIds in ascending order.
    pub fn list_block_ids(&self) -> Vec<BlockId> {
        self.frames
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| entry.as_ref().map(|_| idx as BlockId))
            .collect()
    }

    /// Remove a block (keeps the slot as a hole).
    pub fn delete(&mut self, id: BlockId) -> StoreResult<()> {
        let idx = Self::index_of(id)?;
        let slot = self.frames.get_mut(idx).ok_or(StoreError::OutOfRange(id))?;
        if slot.is_none() {
            return Err(StoreError::OutOfRange(id));
        }
        *slot = None;
        Ok(())
    }
}

impl BlockStore for MemBlockStore {
    fn put_l0(&mut self, raw: &[u8]) -> StoreResult<BlockId> {
        let id = self.next_id();
        let frame = make_frame_l0(id, raw);
        self.frames.push(Some(frame));
        Ok(id)
    }

    fn put_multi(&mut self, recipe: &MultiRecipe) -> StoreResult<BlockId> {
        let id = self.next_id();
        let frame = make_frame_multi(id, recipe);
        self.frames.push(Some(frame));
        Ok(id)
    }

    fn put_z(&mut self, z: &ZPayload) -> StoreResult<BlockId> {
        let id = self.next_id();
        let frame = make_frame_z(id, z);
        self.frames.push(Some(frame));
        Ok(id)
    }

    fn put_object(&mut self, o: &ObjectPayload) -> StoreResult<BlockId> {
        let id = self.next_id();
        let frame = make_frame_object(id, o);
        self.frames.push(Some(frame));
        Ok(id)
    }

    fn get_typed(&self, id: BlockId) -> StoreResult<(BlockKind, [u8; 32], BlockBody)> {
        let frame = self.frame_at(id)?;
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
        let frame = self.frame_at(id)?;
        Ok(frame.to_vec())
    }
}
