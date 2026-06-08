#[cfg(test)]
mod tests {
    use crate::state::AppState;
    use network::messages::{PlayerGameMode, Position, Rotation};

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert_eq!(state.get_seed(), 0);
    }

    #[test]
    fn test_app_state_seed() {
        let state = AppState::new();
        state.init_seed(12345);
        assert_eq!(state.get_seed(), 12345);
    }

    #[test]
    fn test_app_state_add_player() {
        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "TestPlayer".to_string());
        let players = state.get_all_players_vec();
        assert!(players.is_some());
        let players = players.unwrap();
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].id, 1);
    }

    #[test]
    fn test_app_state_player_position() {
        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "TestPlayer".to_string());

        let pos = state.get_player_position(1);
        assert!(pos.is_some());

        let new_pos = Position {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        let new_rot = Rotation { x: 1.0, y: 0.5 };
        state.update_player_position(1, new_pos.clone(), new_rot.clone());

        let updated = state.get_player_position(1).unwrap();
        assert!((updated.x - 10.0).abs() < 1e-6);
        assert!((updated.y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_app_state_remove_player() {
        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "TestPlayer".to_string());
        state.remove_player(&1);
        assert!(state.get_player(1).is_none());
    }

    #[test]
    fn test_app_state_gamemode_change() {
        use network::messages::PlayerGameMode;

        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "TestPlayer".to_string());

        state.set_player_gamemode(1, PlayerGameMode::Spectator);
        let player = state.get_player(1).unwrap();
        assert_eq!(player.gamemode, PlayerGameMode::Spectator);
    }

    #[test]
    fn test_app_state_set_block() {
        let state = AppState::new();
        state.init_seed(42);
        state.set_block(0, 64, 0, 1);
        // Should not panic even without generated chunks
    }

    #[test]
    fn test_app_state_get_player_rotation() {
        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "TestPlayer".to_string());

        let rot = state.get_player_rotation(1);
        assert!(rot.is_some());
        let rot = rot.unwrap();
        assert!((rot.x - 0.0).abs() < 1e-6);
        assert!((rot.y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_player_creation_matches_add() {
        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "Alice".to_string());
        state.add_player(2, "Bob".to_string());

        let alice = state.get_player(1).unwrap();
        assert_eq!(alice.username, "Alice");
        assert_eq!(alice.id, 1);

        let players = state.get_all_players_vec().unwrap();
        assert_eq!(players.len(), 2);
    }

    #[test]
    fn test_multiple_gamemode_changes() {
        let state = AppState::new();
        state.init_seed(42);
        state.add_player(1, "Player".to_string());

        state.set_player_gamemode(1, PlayerGameMode::Spectator);
        assert_eq!(state.get_player(1).unwrap().gamemode, PlayerGameMode::Spectator);

        state.set_player_gamemode(1, PlayerGameMode::Survival);
        assert_eq!(state.get_player(1).unwrap().gamemode, PlayerGameMode::Survival);
    }
}
