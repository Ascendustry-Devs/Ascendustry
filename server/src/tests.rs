#[cfg(test)]
mod tests {
    use crate::state::AppState;
    use game::player::PlayerGameMode;
    use game::types::{Position, Rotation};

    #[tokio::test]
    async fn test_app_state_new() {
        let state = AppState::new();
        assert_eq!(state.get_seed().await, 0);
    }

    #[tokio::test]
    async fn test_app_state_seed() {
        let state = AppState::new();
        state.init_seed(12345).await;
        assert_eq!(state.get_seed().await, 12345);
    }

    #[tokio::test]
    async fn test_app_state_add_player() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "TestPlayer".to_string()).await;
        let players = state.get_all_players_vec().await;
        assert!(players.is_some());
        let players = players.unwrap();
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].id, 1);
    }

    #[tokio::test]
    async fn test_app_state_player_position() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "TestPlayer".to_string()).await;

        let pos = state.get_player_position(1).await;
        assert!(pos.is_some());

        let new_pos = Position {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        let new_rot = Rotation { x: 1.0, y: 0.5 };
        state.update_player_position(1, new_pos.clone(), new_rot.clone()).await;

        let updated = state.get_player_position(1).await.unwrap();
        assert!((updated.x - 10.0).abs() < 1e-6);
        assert!((updated.y - 20.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_app_state_remove_player() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "TestPlayer".to_string()).await;
        state.remove_player(&1).await;
        assert!(state.get_player(1).await.is_none());
    }

    #[tokio::test]
    async fn test_app_state_gamemode_change() {
        use game::player::PlayerGameMode;

        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "TestPlayer".to_string()).await;

        state.set_player_gamemode(1, PlayerGameMode::Spectator).await;
        let player = state.get_player(1).await.unwrap();
        assert_eq!(player.gamemode, PlayerGameMode::Spectator);
    }

    #[tokio::test]
    async fn test_app_state_set_block() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.set_block(0, 64, 0, 1).await;
        // Should not panic even without generated chunks
    }

    #[tokio::test]
    async fn test_app_state_get_player_rotation() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "TestPlayer".to_string()).await;

        let rot = state.get_player_rotation(1).await;
        assert!(rot.is_some());
        let rot = rot.unwrap();
        assert!((rot.x - 0.0).abs() < 1e-6);
        assert!((rot.y - 0.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_player_creation_matches_add() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "Alice".to_string()).await;
        state.add_player(2, "Bob".to_string()).await;

        let alice = state.get_player(1).await.unwrap();
        assert_eq!(alice.username, "Alice");
        assert_eq!(alice.id, 1);

        let players = state.get_all_players_vec().await.unwrap();
        assert_eq!(players.len(), 2);
    }

    #[tokio::test]
    async fn test_multiple_gamemode_changes() {
        let state = AppState::new();
        state.init_seed(42).await;
        state.add_player(1, "Player".to_string()).await;

        state.set_player_gamemode(1, PlayerGameMode::Spectator).await;
        assert_eq!(state.get_player(1).await.unwrap().gamemode, PlayerGameMode::Spectator);

        state.set_player_gamemode(1, PlayerGameMode::Survival).await;
        assert_eq!(state.get_player(1).await.unwrap().gamemode, PlayerGameMode::Survival);
    }
}
