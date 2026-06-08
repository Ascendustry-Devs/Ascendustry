#[cfg(test)]
mod tests {
    use crate::crypto::compute_shared_secret;
    use crate::messages::{
        new_ping_paquet, new_pong_paquet, ContenuPaquet, Paquet, PlayerGameMode, PlayerTransformation, Position, Rotation,
        TypePaquet, CURRENT_VERSION,
    };
    use crate::network_protocol::create_codec;
    use crate::DEFAULT_SERVER_ADDRESS;

    #[test]
    fn test_packet_creation() {
        let packet = Paquet::new(
            TypePaquet::Handshake,
            ContenuPaquet::DonneesConnexion {
                version: 1,
                username: "test".to_string(),
                player_unique_id: 0,
            },
        );
        assert_eq!(packet.type_paquet, TypePaquet::Handshake);
    }

    #[test]
    fn test_packet_serialization_roundtrip() {
        let original = Paquet::new(
            TypePaquet::PlayerTransformation,
            ContenuPaquet::PlayerTransformation {
                data: PlayerTransformation {
                    player_id: 42,
                    position: Position { x: 1.0, y: 2.0, z: 3.0 },
                    rotation: Rotation { x: 0.5, y: 0.1 },
                },
            },
        );

        let encoded = bincode::serialize(&original).unwrap();
        let decoded: Paquet = bincode::deserialize(&encoded).unwrap();

        assert_eq!(original.type_paquet, decoded.type_paquet);
        match decoded.contenu {
            ContenuPaquet::PlayerTransformation { data } => {
                assert_eq!(data.player_id, 42);
                assert!((data.position.x - 1.0).abs() < 1e-6);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_ping_pong_packets() {
        let ping = new_ping_paquet(12345);
        assert_eq!(ping.type_paquet, TypePaquet::Ping);
        match ping.contenu {
            ContenuPaquet::Ping { timestamp } => assert_eq!(timestamp, 12345),
            _ => panic!("Wrong variant"),
        }

        let pong = new_pong_paquet(12345);
        assert_eq!(pong.type_paquet, TypePaquet::Pong);
    }

    #[test]
    fn test_network_error_display() {
        let err = crate::error::NetworkError::Io("connection refused".to_string());
        let display = format!("{}", err);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_encrypted_codec_create() {
        let key = [0xABu8; 32];
        let codec = create_codec(key);
        assert_eq!(codec.cipher().encrypt(b"test").len(), 32);
    }

    #[test]
    fn test_crypto_compute_shared_secret() {
        let server_id = [0x42u8; 16];
        let secret = compute_shared_secret(&server_id, b"server");
        assert_eq!(secret.len(), 32);
    }

    #[test]
    fn test_default_server_address() {
        assert_eq!(DEFAULT_SERVER_ADDRESS, "127.0.0.1:42677");
    }

    #[test]
    fn test_codec_const_exists() {
        let _ = CURRENT_VERSION;
    }

    #[test]
    fn test_player_gamemode_serialization() {
        let modes = [PlayerGameMode::Survival, PlayerGameMode::Spectator];
        for mode in &modes {
            let encoded = bincode::serialize(mode).unwrap();
            let decoded: PlayerGameMode = bincode::deserialize(&encoded).unwrap();
            assert_eq!(*mode, decoded);
        }
    }
}
