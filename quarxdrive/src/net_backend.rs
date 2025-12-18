use std::io;

/// Backend abstraction for the network layer:
/// storage of raw blobs of blocks and objects.
///
/// Can be backed by:
///   - HGSS/QuarXCore repo (real blocks/objects),
///   - RAM-tier,
///   - mixed strategies.
pub trait NetBackend {
    /// Store raw stream of blocks (L0/Multi/Z) as a single blob.
    fn put_blocks_raw(&self, raw: &[u8]) -> io::Result<()>;

    /// Load last stored block stream.
    fn get_blocks_raw(&self) -> io::Result<Vec<u8>>;

    /// Store raw object (blob + tree).
    fn put_object_raw(&self, raw: &[u8]) -> io::Result<()>;

    /// Load last stored object.
    fn get_object_raw(&self) -> io::Result<Vec<u8>>;
}
