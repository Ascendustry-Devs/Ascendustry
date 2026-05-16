//! Tests unifiés pour la bilbiothèque partagée (`shared`).
//!
//! Couvre les modules suivants :
//! - [`network::messages`]      : Sérialisation/désérialisation des paquets
//! - [`network::crypto`]        : Chiffrement AES, secret partagé, hex
//! - [`network::network_protocol`] : Codec chiffré encode/decode
//! - [`network::error`]         : Affichage et conversion des erreurs
//! - [`buffer_pool`]            : Pool de buffers réutilisables
//! - [`world::constants`]       : Constantes du monde
//! - [`world::data::block`]     : Types de blocs et gestionnaire
//! - [`world::data::chunk`]     : Indexation des chunks, conversion de coordonnées
//! - [`world::modified_chunk`]  : Modifications de blocs
//! - [`world::generation`]      : Génération déterministe de chunks

use aes_gcm::{Aes256Gcm, KeyInit};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// network::messages
// ---------------------------------------------------------------------------

/// Vérifie que chaque variante de `ContenuPaquet` fait un aller-retour
/// correct sérialisation → désérialisation.
#[test]
fn messages_serialize_roundtrip_all_variants() {
    use crate::network::messages::*;

    let cases: Vec<Paquet> = vec![
        create_handshake("Alice".to_string()),
        create_handshake_ack(42, 12345),
        create_player_update(1, 10.5, 20.3, -5.0, 0.1, 0.2),
        new_server_seed_paquet(12345),
        new_ping_paquet(1000),
        new_pong_paquet(2000),
        new_set_block_paquet(10, 20, 30, 1),
        Paquet::new(
            TypePaquet::WorldData,
            ContenuPaquet::DonneesMonde {
                chunks: vec![ChunkData {
                    x: 0,
                    y: 0,
                    z: 0,
                    data: vec![1, 2, 3],
                }],
            },
        ),
        Paquet::new(
            TypePaquet::MultiplePlayerTransformation,
            ContenuPaquet::MultiplePlayerTransformation {
                data: vec![PlayerTransformation {
                    player_id: 1,
                    position: Position { x: 1.0, y: 2.0, z: 3.0 },
                    rotation: Rotation { x: 0.5, y: 0.3 },
                }],
            },
        ),
    ];

    for original in cases {
        let bytes = original.serialize();
        let deserialized = Paquet::deserialize(&bytes).unwrap();
        assert_eq!(
            deserialized.type_paquet, original.type_paquet,
            "le type de paquet ne correspond pas apres roundtrip",
        );
    }
}

/// Vérifie que `create_handshake` construit un paquet correct.
#[test]
fn messages_create_handshake() {
    use crate::network::messages::*;
    let p = create_handshake("Alice".to_string());
    assert_eq!(p.type_paquet, TypePaquet::Handshake);
    match &p.contenu {
        ContenuPaquet::DonneesConnexion { version, username } => {
            assert_eq!(*version, CURRENT_VERSION);
            assert_eq!(username, "Alice");
        }
        _ => panic!("type de contenu inattendu"),
    }
}

/// Vérifie que `new_ping_paquet` et `new_pong_paquet` conservent le timestamp.
#[test]
fn messages_ping_pong_timestamp() {
    use crate::network::messages::*;

    let ping = new_ping_paquet(9999);
    match &ping.contenu {
        ContenuPaquet::Ping { timestamp } => assert_eq!(*timestamp, 9999),
        _ => panic!("attendu Ping"),
    }

    let pong = new_pong_paquet(9999);
    match &pong.contenu {
        ContenuPaquet::Pong { timestamp } => assert_eq!(*timestamp, 9999),
        _ => panic!("attendu Pong"),
    }
}

/// Vérifie que `create_player_update` remplit correctement les champs.
#[test]
fn messages_create_player_update() {
    use crate::network::messages::*;
    let p = create_player_update(7, 1.0, 2.0, 3.0, 0.1, 0.2);
    assert_eq!(p.type_paquet, TypePaquet::PlayerTransformation);
    match &p.contenu {
        ContenuPaquet::PlayerTransformation { data } => {
            assert_eq!(data.player_id, 7);
            assert!((data.position.x - 1.0).abs() < f32::EPSILON);
            assert!((data.rotation.x - 0.1).abs() < f32::EPSILON);
        }
        _ => panic!("attendu PlayerTransformation"),
    }
}

/// Vérifie que `new_set_block_paquet` conserve les coordonnées et l'id du bloc.
#[test]
fn messages_set_block() {
    use crate::network::messages::*;
    let p = new_set_block_paquet(-10, 20, -30, 3);
    assert_eq!(p.type_paquet, TypePaquet::SetBlock);
    match &p.contenu {
        ContenuPaquet::SetBlock { x, y, z, block_id } => {
            assert_eq!(*x, -10);
            assert_eq!(*y, 20);
            assert_eq!(*z, -30);
            assert_eq!(*block_id, 3);
        }
        _ => panic!("attendu SetBlock"),
    }
}

/// Vérifie que les chaînes vides et les valeurs extrêmes passent la sérialisation.
#[test]
fn messages_edge_cases() {
    use crate::network::messages::*;

    let p = create_handshake(String::new());
    let bytes = p.serialize();
    let back = Paquet::deserialize(&bytes).unwrap();
    match &back.contenu {
        ContenuPaquet::DonneesConnexion { username, .. } => assert!(username.is_empty()),
        _ => panic!("attendu DonneesConnexion"),
    }

    let p = new_ping_paquet(u64::MAX);
    let bytes = p.serialize();
    let back = Paquet::deserialize(&bytes).unwrap();
    match &back.contenu {
        ContenuPaquet::Ping { timestamp } => assert_eq!(*timestamp, u64::MAX),
        _ => panic!("attendu Ping"),
    }
}

// ---------------------------------------------------------------------------
// network::crypto
// ---------------------------------------------------------------------------

/// Vérifie que `compute_shared_secret` est déterministe : mêmes entrées -> même sortie.
#[test]
fn crypto_shared_secret_deterministic() {
    use crate::network::crypto::compute_shared_secret;

    let server_id = [0xABu8; 16];
    let token = b"server";

    let a = compute_shared_secret(&server_id, token);
    let b = compute_shared_secret(&server_id, token);
    assert_eq!(a, b);
}

/// Vérifie que deux entrées différentes produisent des secrets différents.
#[test]
fn crypto_shared_secret_different_inputs() {
    use crate::network::crypto::compute_shared_secret;

    let a = compute_shared_secret(&[1u8; 16], b"token_a");
    let b = compute_shared_secret(&[2u8; 16], b"token_b");
    assert_ne!(a, b);
}

/// Vérifie que `server_id_to_hex` produit la chaîne hexadécimale attendue.
#[test]
fn crypto_server_id_to_hex() {
    use crate::network::crypto::server_id_to_hex;

    assert_eq!(server_id_to_hex(&[0x00, 0xFF]), "00ff");
    assert_eq!(server_id_to_hex(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
    assert_eq!(server_id_to_hex(&[]), String::new());
}

/// Vérifie que `generate_server_id` produit 16 octets.
#[test]
fn crypto_generate_server_id_length() {
    use crate::network::crypto::generate_server_id;
    let id = generate_server_id();
    assert_eq!(id.len(), 16);
}

/// Vérifie l'aller-retour chiffrement/déchiffrement AES.
#[test]
fn crypto_aes_roundtrip() {
    use crate::network::crypto::{aes_decrypt, aes_encrypt};
    use aes_gcm::Key;

    let key = [0x42u8; 32];
    let key = Key::<Aes256Gcm>::from_slice(&key);
    let cipher = Aes256Gcm::new(key);

    let plaintexts: &[&[u8]] = &[b"", b"hello", &[0u8; 100], &[0xFFu8; 1024]];

    for plain in plaintexts {
        let encrypted = aes_encrypt(plain, &cipher).unwrap();
        let decrypted = aes_decrypt(&encrypted, &cipher).unwrap();
        assert_eq!(&decrypted, plain);
    }
}

/// Vérifie que déchiffrer avec une clé différente échoue (authentification AES-GCM).
#[test]
fn crypto_aes_wrong_key_fails() {
    use crate::network::crypto::{aes_decrypt, aes_encrypt};
    use aes_gcm::Key;

    let key_a = [0xAAu8; 32];
    let key_b = [0xBBu8; 32];
    let cipher_a = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_a));
    let cipher_b = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_b));

    let encrypted = aes_encrypt(b"secret data", &cipher_a).unwrap();
    let result = aes_decrypt(&encrypted, &cipher_b);
    assert!(result.is_err());
}

/// Vérifie que déchiffrer des données trop courtes (< 12 octets) échoue.
#[test]
fn crypto_aes_decrypt_too_short() {
    use crate::network::crypto::aes_decrypt;
    use aes_gcm::Key;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&[0u8; 32]));
    let result = aes_decrypt(&[0u8; 6], &cipher);
    assert!(result.is_err());
}

/// Vérifie que `xor_crypt` est sa propre inverse.
#[test]
fn crypto_xor_roundtrip() {
    use crate::network::crypto::xor_crypt;

    let data = b"Hello, world!";
    let key = b"secret";
    let encrypted = xor_crypt(data, key);
    let decrypted = xor_crypt(&encrypted, key);
    assert_eq!(&decrypted, data);
}

/// Vérifie que `xor_crypt` fonctionne avec une clé à un seul octet.
#[test]
fn crypto_xor_single_byte_key() {
    use crate::network::crypto::xor_crypt;

    let data = b"test data";
    let key = b"\xFF";
    let encrypted = xor_crypt(data, key);
    let decrypted = xor_crypt(&encrypted, key);
    assert_eq!(&decrypted, data);
    assert_ne!(&encrypted, data);
}

// ---------------------------------------------------------------------------
// network::network_protocol  (EncryptedCodec, Cipher, handshake helpers)
// ---------------------------------------------------------------------------

/// Vérifie que `Cipher::encrypt` / `decrypt` font un aller-retour correct.
#[test]
fn protocol_cipher_roundtrip() {
    use crate::network::network_protocol::Cipher;

    let cipher = Cipher::new([0x42u8; 32]);
    let data = b"sensitive data to encrypt";
    let encrypted = cipher.encrypt(data);
    let decrypted = cipher.decrypt(&encrypted);
    assert_eq!(&decrypted, data);
}

/// Vérifie l'aller-retour `EncryptedCodec::encode` -> `decode` pour chaque
/// variante de paquet.
#[test]
fn protocol_codec_encode_decode_roundtrip() {
    use crate::network::messages::*;
    use crate::network::network_protocol::create_codec;

    let codec = create_codec([0x42u8; 32]);

    let packets = vec![
        create_handshake("Bob".to_string()),
        create_handshake_ack(1, 0),
        create_player_update(2, 1.0, 2.0, 3.0, 0.1, 0.2),
        new_server_seed_paquet(9999),
        new_ping_paquet(5555),
        new_pong_paquet(6666),
        new_set_block_paquet(10, 20, 30, 2),
    ];

    for original in packets {
        let encoded = codec.encode(&original);
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(
            decoded.type_paquet, original.type_paquet,
            "le type ne correspond pas apres encode/decode",
        );
    }
}

/// Vérifie que `decode` panique sur des données invalides (le decrypt expecte).
#[test]
fn protocol_codec_decode_invalid_data_panics() {
    use crate::network::network_protocol::create_codec;

    let codec = create_codec([0x42u8; 32]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = codec.decode(b"garbage");
    }));
    assert!(result.is_err());
}

/// Vérifie que `perform_server_handshake` et `perform_client_handshake`
/// produisent le même secret quand le client utilise le token "server".
#[test]
fn protocol_handshake_match() {
    use crate::network::network_protocol::{perform_client_handshake, perform_server_handshake};

    let server_id = [0x42u8; 16];
    let server_secret = perform_server_handshake(&server_id);
    let client_secret = perform_client_handshake(&server_id, b"server");
    assert_eq!(server_secret, client_secret);
}

/// Vérifie que `create_server_id` retourne 16 octets et une chaîne hex de 32 chars.
#[test]
fn protocol_create_server_id() {
    use crate::network::network_protocol::create_server_id;

    let (id, hex) = create_server_id();
    assert_eq!(id.len(), 16);
    assert_eq!(hex.len(), 32);
}

/// Vérifie que `create_cipher` produit un cipher utilisable.
#[test]
fn protocol_create_cipher_usable() {
    use crate::network::network_protocol::create_cipher;

    let cipher = create_cipher([0u8; 32]);
    let data = b"test";
    let encrypted = cipher.encrypt(data);
    let decrypted = cipher.decrypt(&encrypted);
    assert_eq!(&decrypted, data);
}

// ---------------------------------------------------------------------------
// network::error
// ---------------------------------------------------------------------------

/// Vérifie le format d'affichage de chaque variante de `NetworkError`.
#[test]
fn error_display_format() {
    use crate::network::error::NetworkError;

    let cases: Vec<(NetworkError, &str)> = vec![
        (NetworkError::Io("timeout".into()), "IO error: timeout"),
        (NetworkError::Codec("bad format".into()), "Codec error: bad format"),
        (NetworkError::PacketTooLarge(99999), "Packet too large: 99999 bytes"),
        (NetworkError::InvalidPacket("bad checksum".into()), "Invalid packet: bad checksum"),
        (NetworkError::Disconnected, "Connection closed"),
        (NetworkError::NotConnected, "Not connected"),
        (NetworkError::InvalidData("truncated".into()), "Invalid data: truncated"),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

/// Vérifie les conversions `From` vers `NetworkError`.
#[test]
fn error_from_conversions() {
    use crate::network::error::NetworkError;

    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "custom io");
    let err: NetworkError = io_err.into();
    assert!(matches!(err, NetworkError::Io(_)));

    let err: NetworkError = "string error".to_string().into();
    assert!(matches!(err, NetworkError::Codec(_)));

    let err: NetworkError = "str error".into();
    assert!(matches!(err, NetworkError::Codec(_)));
}

/// Vérifie que `PacketTooLarge` préserve la taille après clone.
#[test]
fn error_clone_preserves_size() {
    use crate::network::error::NetworkError;

    let original = NetworkError::PacketTooLarge(70000);
    let cloned = original.clone();
    match cloned {
        NetworkError::PacketTooLarge(size) => assert_eq!(size, 70000),
        _ => panic!("PacketTooLarge attendu"),
    }
}

// ---------------------------------------------------------------------------
// buffer_pool
// ---------------------------------------------------------------------------

/// Vérifie le cycle complet : acquérir un buffer, le relâcher, le ré-acquérir.
#[test]
fn buffer_pool_acquire_release() {
    use crate::buffer_pool::BufferPool;

    let pool: BufferPool<i32> = BufferPool::new(32);

    let mut buf = pool.get_buffer();
    assert!(buf.capacity() >= 32);
    buf.push(42);

    pool.release_buffer(buf);

    let buf2 = pool.get_buffer();
    assert!(buf2.is_empty());
    assert!(buf2.capacity() >= 32);
}

/// Vérifie que plusieurs buffers peuvent être recyclés.
#[test]
fn buffer_pool_multiple_buffers() {
    use crate::buffer_pool::BufferPool;

    let pool: BufferPool<i32> = BufferPool::new(10);
    let b1 = pool.get_buffer();
    let b2 = pool.get_buffer();
    let b3 = pool.get_buffer();

    pool.release_buffer(b1);
    pool.release_buffer(b2);
    pool.release_buffer(b3);

    let _r1 = pool.get_buffer();
    let _r2 = pool.get_buffer();
    let _r3 = pool.get_buffer();
}

/// Vérifie que le pool fonctionne avec différents types.
#[test]
fn buffer_pool_different_types() {
    use crate::buffer_pool::BufferPool;

    let pool_f32: BufferPool<f32> = BufferPool::new(8);
    let buf = pool_f32.get_buffer();
    assert!(buf.capacity() >= 8);

    let pool_string: BufferPool<String> = BufferPool::new(4);
    let buf = pool_string.get_buffer();
    assert!(buf.capacity() >= 4);
}

// ---------------------------------------------------------------------------
// world::constants
// ---------------------------------------------------------------------------

/// Vérifie les valeurs des constantes de rendu et simulation.
#[test]
fn constants_render_distances() {
    use crate::world::constants::*;

    assert_eq!(HORIZONTAL_RENDER_DISTANCE, 9);
    assert_eq!(VERTICAL_RENDER_DISTANCE, 5);
    assert_eq!(HORIZONTAL_SIMULATION_DISTANCE, 11);
    assert_eq!(VERTICAL_SIMULATION_DISTANCE, 7);
}

/// Vérifie les constantes de priorité des chunks.
#[test]
fn constants_chunk_priority() {
    use crate::world::constants::*;

    assert_eq!(CHUNK_PRIORITY_DISTANCE, 32.0);
    assert_eq!(CHUNK_PRIORITY_DISTANCE_SQR, 1024.0);
}

/// Vérifie que `max_chunks_in_queue` calcule correctement 11 x 11 x 7 = 847.
#[test]
fn constants_max_chunks_in_queue() {
    use crate::world::constants::max_chunks_in_queue;
    assert_eq!(max_chunks_in_queue(), 847);
}

// ---------------------------------------------------------------------------
// world::data::block
// ---------------------------------------------------------------------------

/// Vérifie les conversions `BlockType::from_id` pour chaque valeur.
#[test]
fn block_blocktype_from_id() {
    use crate::world::data::block::BlockType;

    assert!(BlockType::from_id(0) == BlockType::Air);
    assert!(BlockType::from_id(1) == BlockType::Grass);
    assert!(BlockType::from_id(2) == BlockType::Dirt);
    assert!(BlockType::from_id(3) == BlockType::Stone);
    assert!(BlockType::from_id(4) == BlockType::Dirt);
    assert!(BlockType::from_id(999) == BlockType::Dirt);
}

/// Vérifie les indices de texture de chaque type de bloc.
#[test]
fn block_blocktype_texture_index() {
    use crate::world::data::block::BlockType;

    assert_eq!(BlockType::Air.texture_index(), 0);
    assert_eq!(BlockType::Grass.texture_index(), 0);
    assert_eq!(BlockType::Dirt.texture_index(), 1);
    assert_eq!(BlockType::Stone.texture_index(), 2);
}

/// Vérifie les propriétés de base de `BlockInstance`.
#[test]
fn block_instance_basics() {
    use crate::world::data::block::BlockInstance;

    let air = BlockInstance::air();
    assert!(air.is_air());
    assert!(!air.is_solid());
    assert_eq!(air.id, 0);

    let stone = BlockInstance::new(3);
    assert!(!stone.is_air());
    assert!(stone.is_solid());
    assert_eq!(stone.block_type().to_u32(), 3);
}

/// Vérifie que `BlockManager::register` et les lookups fonctionnent.
#[test]
fn block_manager_register_and_lookup() {
    use crate::world::data::block::{BlockData, BlockManager};

    let mut bm = BlockManager::new();
    assert_eq!(bm.block_count(), 0);

    bm.register(BlockData::new("stone"));
    assert_eq!(bm.block_count(), 1);

    let stone = bm.get_block_by_id(0).unwrap();
    assert_eq!(stone.get_id_str(), "stone");

    let stone_by_str = bm.get_block_by_string("stone".to_string()).unwrap();
    assert_eq!(stone_by_str.get_id(), 0);
}

/// Vérifie que `get_block_by_string` retourne `None` pour un nom inconnu.
#[test]
fn block_manager_lookup_unknown() {
    use crate::world::data::block::BlockManager;

    let bm = BlockManager::new();
    assert!(bm.get_block_by_string("unknown".to_string()).is_none());
    assert!(bm.get_block_by_id(0).is_none());
}

/// Vérifie que `dispose` vide complètement le gestionnaire.
#[test]
fn block_manager_dispose() {
    use crate::world::data::block::{BlockData, BlockManager};

    let mut bm = BlockManager::new();
    bm.register(BlockData::new("stone"));
    bm.dispose();
    assert_eq!(bm.block_count(), 0);
    assert!(bm.get_block_by_id(0).is_none());
}

// ---------------------------------------------------------------------------
// world::data::chunk
// ---------------------------------------------------------------------------

/// Vérifie que les constantes de chunk ont les valeurs attendues.
#[test]
fn chunk_constants() {
    use crate::world::data::chunk::*;

    assert_eq!(CHUNK_SIZE, 32);
    assert_eq!(CHUNK_SIZE_F, 32.0);
    assert_eq!(CHUNK_SIZE_HALFED, 16);
    assert_eq!(CHUNK_SIZE_SQR, 1024);
    assert_eq!(CHUNK_BLOCK_NUMBER, 32768);
    assert_eq!(LAST_CHUNK_AXIS_INDEX, 31);
}

/// Vérifie que la formule d'indexation linéaire est correcte pour les 8 coins.
#[test]
fn chunk_linear_index_corners() {
    use crate::world::data::block::BlockInstance;
    use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER, CHUNK_SIZE, CHUNK_SIZE_SQR};

    let mut chunk = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 0,
        y: 0,
        z: 0,
    };

    let corners = [
        (0, 0, 0),
        (31, 0, 0),
        (0, 31, 0),
        (31, 31, 0),
        (0, 0, 31),
        (31, 0, 31),
        (0, 31, 31),
        (31, 31, 31),
    ];

    for (i, &(x, y, z)) in corners.iter().enumerate() {
        let id = (i + 1) as u32;
        chunk.set_block_from_xyz(x, y, z, BlockInstance::new(id));
        let expected_index = (x + y * CHUNK_SIZE + z * CHUNK_SIZE_SQR) as usize;
        assert_eq!(chunk.get_block_from_i(expected_index).id, id);
        assert_eq!(chunk.get_block_from_xyz(x, y, z).id, id);
    }
}

/// Vérifie que `get_block_from_xyz` et `set_block_from_xyz` sont cohérents.
#[test]
fn chunk_set_get_roundtrip() {
    use crate::world::data::block::BlockInstance;
    use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER};

    let mut chunk = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 0,
        y: 0,
        z: 0,
    };

    let test_positions = [(5, 10, 20), (0, 0, 0), (31, 31, 31), (15, 15, 15)];
    for (x, y, z) in test_positions {
        chunk.set_block_from_xyz(x, y, z, BlockInstance::new(2));
        assert_eq!(chunk.get_block_from_xyz(x, y, z).id, 2);
    }
}

/// Vérifie que `compute_checksum` est déterministe.
#[test]
fn chunk_checksum_deterministic() {
    use crate::world::data::block::BlockInstance;
    use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER};

    let chunk = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 1,
        y: 2,
        z: 3,
    };

    let a = chunk.compute_checksum();
    let b = chunk.compute_checksum();
    assert_eq!(a, b);
}

/// Vérifie que deux chunks différents ont des checksums différents.
#[test]
fn chunk_checksum_different() {
    use crate::world::data::block::BlockInstance;
    use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER};

    let a = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 0,
        y: 0,
        z: 0,
    };
    let b = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 1,
        y: 0,
        z: 0,
    };
    assert_ne!(a.compute_checksum(), b.compute_checksum());
}

/// Vérifie les conversions de coordonnées globales -> chunk pour divers cas.
///
/// Note : la fonction utilise `/` (troncature vers zéro), ce qui signifie
/// que -1/32 = 0, -32/32 = -1, -33/32 = -1.
#[test]
fn chunk_global_position_to_chunk_pos() {
    use crate::world::data::chunk::global_position_to_chunk_pos;

    let (chunk, _intra) = global_position_to_chunk_pos(0, 0, 0);
    assert_eq!(chunk, (0, 0, 0));

    let (chunk, _intra) = global_position_to_chunk_pos(31, 31, 31);
    assert_eq!(chunk, (0, 0, 0));

    let (chunk, _intra) = global_position_to_chunk_pos(32, 32, 32);
    assert_eq!(chunk, (1, 1, 1));

    let (chunk, _intra) = global_position_to_chunk_pos(-1, -1, -1);
    assert_eq!(chunk, (-1, -1, -1));

    let (chunk, _intra) = global_position_to_chunk_pos(-32, -32, -32);
    assert_eq!(chunk, (-1, -1, -1));

    let (chunk, _intra) = global_position_to_chunk_pos(-33, -33, -33);
    assert_eq!(chunk, (-2, -2, -2));
}

/// Vérifie que `ChunkData::new` initialise correctement les champs.
#[test]
fn chunk_data_new() {
    use crate::world::data::block::BlockInstance;
    use crate::world::data::chunk::{Chunk, ChunkData, CHUNK_BLOCK_NUMBER};

    let chunk = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 0,
        y: 0,
        z: 0,
    };
    let data = ChunkData::new(chunk);
    assert!(data.is_dirty);
}

/// Vérifie que `set_dirty` fonctionne.
#[test]
fn chunk_data_set_dirty() {
    use crate::world::data::block::BlockInstance;
    use crate::world::data::chunk::{Chunk, ChunkData, CHUNK_BLOCK_NUMBER};

    let chunk = Chunk {
        blocks: vec![BlockInstance::air(); CHUNK_BLOCK_NUMBER],
        x: 0,
        y: 0,
        z: 0,
    };
    let mut data = ChunkData::new(chunk);
    data.is_dirty = false;

    data.set_dirty();
    assert!(data.is_dirty);
}

// ---------------------------------------------------------------------------
// world::modified_chunk (via ModifiedWorld)
// ---------------------------------------------------------------------------

/// Vérifie que `ModifiedWorld::set_block_at` crée automatiquement le chunk
/// et que `get_block_at` retrouve le bloc.
#[test]
fn modified_world_set_get_block() {
    use crate::world::data::block::BlockInstance;
    use crate::world::modified_chunk::ModifiedWorld;

    let mut mw = ModifiedWorld::new();
    mw.set_block_at(10, 20, 30, BlockInstance::new(3));

    let block = mw.get_block_at(10, 20, 30);
    assert!(block.is_some());
    assert_eq!(block.unwrap().id, 3);
}

/// Vérifie que `get_chunk_at` retourne une erreur pour un chunk inexistant.
#[test]
fn modified_world_get_chunk_at_error() {
    use crate::world::modified_chunk::{ModifiedWorld, ModifiedWorldError};

    let mw = ModifiedWorld::new();
    let result = mw.get_chunk_at(99, 99, 99);
    assert!(matches!(result, Err(ModifiedWorldError::ValeurInvalide(99, 99, 99))));
}

/// Vérifie que `retain_chunks` supprime les chunks non conservés.
#[test]
fn modified_world_retain_chunks() {
    use crate::world::data::block::BlockInstance;
    use crate::world::modified_chunk::ModifiedWorld;

    let mut mw = ModifiedWorld::new();
    mw.set_block_at(0, 0, 0, BlockInstance::new(1));
    mw.set_block_at(32, 32, 32, BlockInstance::new(2));
    mw.set_block_at(64, 64, 64, BlockInstance::new(3));

    let keep: std::collections::HashSet<(i32, i32, i32)> = [(0, 0, 0), (1, 1, 1)].into();
    mw.retain_chunks(&keep);

    assert!(mw.get_block_at(0, 0, 0).is_some());
    assert!(mw.get_block_at(32, 32, 32).is_some());
    assert!(mw.get_block_at(64, 64, 64).is_none());
}

/// Vérifie que `remove_block_at` retourne le bloc supprimé.
#[test]
fn modified_world_remove_block() {
    use crate::world::data::block::BlockInstance;
    use crate::world::modified_chunk::ModifiedWorld;

    let mut mw = ModifiedWorld::new();
    mw.set_block_at(5, 5, 5, BlockInstance::new(42));

    let removed = mw.remove_block_at(5, 5, 5);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 42);
    assert!(mw.get_block_at(5, 5, 5).is_none());
}

// ---------------------------------------------------------------------------
// world::generation
// ---------------------------------------------------------------------------

/// Vérifie que `is_cave_block` retourne `false` pour une profondeur < CAVE_MIN_DEPTH.
#[test]
fn generation_is_cave_block_shallow() {
    use crate::world::data::block::BlockManager;
    use crate::world::generation::chunk_generator::ChunkGenContext;

    let bm = Arc::new(BlockManager::new());
    let ctx = ChunkGenContext::new(42, bm);

    assert!(!ctx.is_cave_block(0.0, 0.0, 0.0, -1));
    assert!(!ctx.is_cave_block(100.0, 100.0, 100.0, -5));
}

/// Vérifie que `ChunkGenContext::new` initialise la seed.
#[test]
fn generation_context_seeds() {
    use crate::world::data::block::BlockManager;
    use crate::world::generation::chunk_generator::ChunkGenContext;

    let bm = Arc::new(BlockManager::new());
    let ctx = ChunkGenContext::new(42, bm);

    assert_eq!(ctx.seed, 42);
}

/// Vérifie que la génération d'un chunk est déterministe : même seed + mêmes
/// coordonnées -> même résultat.
#[test]
fn generation_chunk_deterministic() {
    use crate::world::data::block::{BlockData, BlockManager};
    use crate::world::data::chunk::{Chunk, CHUNK_BLOCK_NUMBER};
    use crate::world::generation::chunk_generator::ChunkGenContext;

    let mut bm = BlockManager::new();
    for name in &["air", "stone", "dirt", "grass"] {
        bm.register(BlockData::new(name));
    }
    let bm = Arc::new(bm);

    let ctx = ChunkGenContext::new(42, Arc::clone(&bm));

    let a = Chunk::generate_with_context(0, 0, 0, &ctx);
    let b = Chunk::generate_with_context(0, 0, 0, &ctx);

    assert_eq!(a.compute_checksum(), b.compute_checksum());
    for i in 0..CHUNK_BLOCK_NUMBER {
        assert!(a.blocks[i] == b.blocks[i]);
    }
}

/// Vérifie que deux seeds différentes produisent des chunks différents.
#[test]
fn generation_chunk_different_seeds() {
    use crate::world::data::block::{BlockData, BlockManager};
    use crate::world::data::chunk::Chunk;
    use crate::world::generation::chunk_generator::ChunkGenContext;

    let mut bm = BlockManager::new();
    for name in &["air", "stone", "dirt", "grass"] {
        bm.register(BlockData::new(name));
    }
    let bm = Arc::new(bm);

    let ctx_a = ChunkGenContext::new(1, Arc::clone(&bm));
    let ctx_b = ChunkGenContext::new(2, Arc::clone(&bm));

    let a = Chunk::generate_with_context(0, 0, 0, &ctx_a);
    let b = Chunk::generate_with_context(0, 0, 0, &ctx_b);

    assert_ne!(a.compute_checksum(), b.compute_checksum());
}

/// Vérifie que `generate_chunks_sequential` produit le nombre attendu de chunks
/// et que chacun a un checksum valide.
#[test]
fn generation_sequential() {
    use crate::world::data::block::{BlockData, BlockManager};
    use crate::world::generation::chunk_generator::generate_chunks_sequential;

    let mut bm = BlockManager::new();
    for name in &["air", "stone", "dirt", "grass"] {
        bm.register(BlockData::new(name));
    }
    let bm = Arc::new(bm);

    let coords = vec![(0, 0, 0), (1, 0, 0), (0, 1, 0)];
    let result = generate_chunks_sequential(bm, 42, coords);

    assert_eq!(result.len(), 3);
    for (_pos, chunk) in &result {
        let expected_checksum = chunk.chunk_data.chunk.compute_checksum();
        assert_eq!(chunk.checksum, expected_checksum);
    }
}

/// Vérifie la stratification du terrain : bloc de surface en herbe,
/// terre en dessous, pierre plus profond.
#[test]
fn generation_terrain_stratification() {
    use crate::world::data::block::{BlockData, BlockManager};
    use crate::world::data::chunk::Chunk;
    use crate::world::generation::chunk_generator::ChunkGenContext;

    let mut bm = BlockManager::new();
    for name in &["air", "stone", "dirt", "grass"] {
        bm.register(BlockData::new(name));
    }
    let bm = Arc::new(bm);

    let ctx = ChunkGenContext::new(42, Arc::clone(&bm));

    let chunk = Chunk::generate_with_context(0, 0, 0, &ctx);

    let grass_id = bm.get_block_by_string("grass".to_string()).unwrap().get_id();
    let dirt_id = bm.get_block_by_string("dirt".to_string()).unwrap().get_id();
    let stone_id = bm.get_block_by_string("stone".to_string()).unwrap().get_id();
    let air_id = bm.get_block_by_string("air".to_string()).unwrap().get_id();

    for x in [0, 15, 31] {
        for z in [0, 15, 31] {
            let mut surface_y = None;
            for y in (0..32).rev() {
                let block = chunk.get_block_from_xyz(x, y, z);
                if block.id != air_id {
                    surface_y = Some(y);
                    break;
                }
            }

            if let Some(sy) = surface_y {
                assert_eq!(
                    chunk.get_block_from_xyz(x, sy, z).id,
                    grass_id,
                    "surface en ({}, {}, {}) devrait etre de l'herbe",
                    x,
                    sy,
                    z,
                );
                if sy >= 3 {
                    assert_eq!(
                        chunk.get_block_from_xyz(x, sy - 3, z).id,
                        dirt_id,
                        "sous-sol en ({}, {}, {}) devrait etre de la terre",
                        x,
                        sy - 3,
                        z,
                    );
                }
                if sy >= 8 {
                    assert_eq!(
                        chunk.get_block_from_xyz(x, sy - 6, z).id,
                        stone_id,
                        "profondeur en ({}, {}, {}) devrait etre de la pierre",
                        x,
                        sy - 6,
                        z,
                    );
                }
            }
        }
    }
}
