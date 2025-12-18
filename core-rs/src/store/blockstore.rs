use crate::block::multi::MultiRecipe;
use crate::codec::{
    encode_l0_raw, encode_multi_recipe, encode_object_payload, encode_z_payload, ObjectPayload,
    ZPayload,
};
use crate::store::decode::{decode_block_typed, BlockBody};
use crate::store::encode::encode_block;
use crate::types::{BlockId, BlockKind};

use crate::net_core::error::NetError;

use blake3::hash;

/// Storage-level errors.
#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Decode(NetError),
    OutOfRange(BlockId),
    Corrupt(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<NetError> for StoreError {
    fn from(e: NetError) -> Self {
        StoreError::Decode(e)
    }
}

/// Universal block store API.
pub trait BlockStore {
    /// Write L0 block (raw bytes) and get its BlockId.
    fn put_l0(&mut self, raw: &[u8]) -> StoreResult<BlockId>;

    /// Write Multi block by recipe.
    fn put_multi(&mut self, recipe: &MultiRecipe) -> StoreResult<BlockId>;

    /// Write Z block (aggregate L0 range).
    fn put_z(&mut self, z: &ZPayload) -> StoreResult<BlockId>;

    /// Write Object block (object DAG).
    fn put_object(&mut self, o: &ObjectPayload) -> StoreResult<BlockId>;

    /// Read typed block.
    fn get_typed(&self, id: BlockId) -> StoreResult<(BlockKind, [u8; 32], BlockBody)>;

    /// Read raw frame as bytes.
    fn get_frame(&self, id: BlockId) -> StoreResult<Vec<u8>>;
}

/// Helper: blake3(payload) -> [u8;32]
fn hash_payload(payload: &[u8]) -> [u8; 32] {
    let h = hash(payload);
    let mut out = [0u8; 32];
    out.copy_from_slice(h.as_bytes());
    out
}

/// Helper for types that build Frame inline.
pub fn make_frame_l0(id: BlockId, raw: &[u8]) -> Vec<u8> {
    let payload = encode_l0_raw(raw);
    let h = hash_payload(&payload);
    encode_block(BlockKind::L0, id, &h, &payload)
}

pub fn make_frame_multi(id: BlockId, recipe: &MultiRecipe) -> Vec<u8> {
    let payload = encode_multi_recipe(recipe);
    let h = hash_payload(&payload);
    encode_block(BlockKind::Multi, id, &h, &payload)
}

pub fn make_frame_z(id: BlockId, z: &ZPayload) -> Vec<u8> {
    let payload = encode_z_payload(z);
    let h = hash_payload(&payload);
    encode_block(BlockKind::Z, id, &h, &payload)
}

pub fn make_frame_object(id: BlockId, o: &ObjectPayload) -> Vec<u8> {
    let payload = encode_object_payload(o);
    let h = hash_payload(&payload);
    encode_block(BlockKind::Object, id, &h, &payload)
}

/// Universal decode from raw frame into typed block body.
pub fn decode_frame_typed(buf: &[u8]) -> StoreResult<(BlockKind, BlockId, [u8; 32], BlockBody)> {
    let (kind, id, hash, body) = decode_block_typed(buf)?;
    Ok((kind, id, hash, body))
}
