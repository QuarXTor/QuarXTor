use smallvec::smallvec;

use quarxtor_core::block::multi::MultiRecipe;
use quarxtor_core::codec::{ObjectPayload, ZPayload};
use quarxtor_core::store::blockstore::BlockStore;
use quarxtor_core::store::decode::BlockBody;
use quarxtor_core::store::mem_store::MemBlockStore;
use quarxtor_core::types::{BlockKind, BlockRef};

#[test]
fn mem_block_store_roundtrip() {
    let mut store = MemBlockStore::new();

    // L0
    let id_l0 = store.put_l0(b"hello-l0").expect("put_l0");

    // Multi (Aggregate)
    let recipe = quarxtor_core::block::multi::MultiRecipe::Aggregate {
        blocks: smallvec![id_l0],
    };
    let id_multi = store.put_multi(&recipe).expect("put_multi");

    // Z block
    let z = ZPayload {
        first_l0: id_l0,
        last_l0: id_l0,
        z_type: 1,
        meta: Vec::new(),
    };
    let id_z = store.put_z(&z).expect("put_z");

    // Object
    let o = ObjectPayload {
        root: BlockRef::Multi(id_multi),
        obj_type: 42,
        meta: b"obj-meta".to_vec(),
    };
    let id_obj = store.put_object(&o).expect("put_object");

    // Read back as typed blocks
    let (k0, _h0, b0) = store.get_typed(id_l0).expect("get_typed l0");
    assert!(matches!(k0, BlockKind::L0));
    match b0 {
        BlockBody::L0(raw) => assert_eq!(raw, b"hello-l0"),
        _ => panic!("expected L0 body"),
    }

    let (k1, _h1, b1) = store.get_typed(id_multi).expect("get_typed multi");
    assert!(matches!(k1, BlockKind::Multi));
    match b1 {
        BlockBody::Multi(recipe2) => match recipe2 {
            MultiRecipe::Aggregate { blocks } => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks[0], id_l0);
            }
            _ => panic!("expected Aggregate recipe"),
        },
        _ => panic!("expected Multi body"),
    }

    let (k2, _h2, b2) = store.get_typed(id_z).expect("get_typed z");
    assert!(matches!(k2, BlockKind::Z));
    match b2 {
        BlockBody::Z(z2) => {
            assert_eq!(z2.first_l0, id_l0);
            assert_eq!(z2.last_l0, id_l0);
            assert_eq!(z2.z_type, 1);
        }
        _ => panic!("expected Z body"),
    }

    let (k3, _h3, b3) = store.get_typed(id_obj).expect("get_typed obj");
    assert!(matches!(k3, BlockKind::Object));
    match b3 {
        BlockBody::Object(o2) => {
            match o2.root {
                BlockRef::Multi(mid) => assert_eq!(mid, id_multi),
                _ => panic!("expected root Multi(id_multi)"),
            }
            assert_eq!(o2.obj_type, 42);
            assert_eq!(o2.meta, b"obj-meta".to_vec());
        }
        _ => panic!("expected Object body"),
    }
}

#[test]
fn mem_block_store_list_block_ids_is_deterministic() {
    let mut store = MemBlockStore::new();

    let id_a = store.put_l0(b"a").expect("put_l0 a");
    let id_b = store.put_l0(b"b").expect("put_l0 b");
    let id_c = store.put_l0(b"c").expect("put_l0 c");

    assert_eq!(store.list_block_ids(), vec![id_a, id_b, id_c]);

    store.delete(id_b).expect("delete b");

    let ids_after = store.list_block_ids();
    assert_eq!(ids_after, vec![id_a, id_c]);
    assert_eq!(ids_after, store.list_block_ids());
    assert!(store.get_frame(id_b).is_err());
}
