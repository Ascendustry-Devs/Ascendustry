//! Tests unifiés pour les fonctionnalités principales du serveur.
//!
//! Ce fichier regroupe tous les tests unitaires du serveur répartis en quatre sections :
//! - [`player`] : Tests du `PlayerRegistry` (ajout, suppression, position, chunks).
//! - [`world`]  : Tests du `WorldState` (seed, génération, rétention, modification de blocs).
//! - [`state`]  : Tests de l'`AppState` (état partagé, cycle de vie des joueurs).
//! - [`handler`]: Tests du `ProductionHandler` (dispatch des paquets, broadcast).

use crate::game::handler::{HandlerContext, PacketHandler, ProductionHandler};
use crate::player::PlayerRegistry;
use crate::state::AppState;
use crate::world::WorldState;
use cgmath::Point3;
use network::messages::*;
use satiscore::constants::{SPAWN_POSITION_X, SPAWN_POSITION_Y, SPAWN_POSITION_Z};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Section 1 : PlayerRegistry
// ---------------------------------------------------------------------------

/// Vérifie l'ajout d'un joueur : ses champs par défaut (position spawn y=64).
#[test]
fn player_add_player() {
    let mut registry = PlayerRegistry::new();
    registry.add(1, "Alice".to_string());

    let player = registry.get(&1).unwrap();
    assert_eq!(player.id, 1);
    assert_eq!(player.username, "Alice");
    assert_eq!(player.position.x, SPAWN_POSITION_X);
    assert_eq!(player.position.y, SPAWN_POSITION_Y);
    assert_eq!(player.position.z, SPAWN_POSITION_Z);
}

/// Vérifie la suppression d'un joueur et la préservation des autres.
#[test]
fn player_remove_player() {
    let mut registry = PlayerRegistry::new();
    registry.add(1, "Alice".to_string());
    registry.add(2, "Bob".to_string());

    let removed = registry.remove(&1);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, 1);
    assert!(registry.get(&1).is_none());
    assert!(registry.get(&2).is_some());
}

/// Vérifie que la suppression d'un joueur inexistant ne cause pas d'erreur.
#[test]
fn player_remove_nonexistent() {
    let mut registry = PlayerRegistry::new();
    assert!(registry.remove(&99).is_none());
}

/// Vérifie que `get_all` retourne tous les joueurs enregistrés.
#[test]
fn player_get_all() {
    let mut registry = PlayerRegistry::new();
    registry.add(1, "Alice".to_string());
    registry.add(2, "Bob".to_string());

    let all = registry.get_all().unwrap();
    assert_eq!(all.len(), 2);
    let ids: Vec<u64> = all.iter().map(|p| p.id).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
}

/// Vérifie que `get_all` retourne une liste vide quand il n'y a aucun joueur.
#[test]
fn player_get_all_empty() {
    let registry = PlayerRegistry::new();
    assert!(registry.get_all().unwrap().is_empty());
}

/// Vérifie que `update_position` met à jour la position/rotation
/// et retourne les coordonnées du chunk contenant la nouvelle position.
#[test]
fn player_update_position() {
    let mut registry = PlayerRegistry::new();
    registry.add(1, "Alice".to_string());

    let pos = Position {
        x: 35.0,
        y: 70.0,
        z: -30.0,
    };
    let rot = Rotation { x: 0.5, y: 0.3 };
    let (cx, cy, cz) = registry.update_position(1, pos, rot);

    // 35.0 / 32.0 = 1.09 → floor = 1
    assert_eq!(cx, 1);
    // 70.0 / 32.0 = 2.18 → floor = 2
    assert_eq!(cy, 2);
    // -30.0 / 32.0 = -0.93 → floor = -1
    assert_eq!(cz, -1);

    let player = registry.get(&1).unwrap();
    assert_eq!(player.position.x, 35.0);
    assert_eq!(player.position.y, 70.0);
    assert_eq!(player.position.z, -30.0);
    assert_eq!(player.rotation.x, 0.5);
    assert_eq!(player.rotation.y, 0.3);
}

/// Vérifie que `update_position` sur un joueur inexistant ne crée pas d'entrée
/// et retourne (0, 0, 0) comme coordonnées de chunk par défaut.
#[test]
fn player_update_position_nonexistent() {
    let mut registry = PlayerRegistry::new();
    let (cx, cy, cz) = registry.update_position(99, Position { x: 10.0, y: 20.0, z: 30.0 }, Rotation { x: 0.0, y: 0.0 });
    assert_eq!(cx, 0);
    assert_eq!(cy, 0);
    assert_eq!(cz, 0);
}

/// Vérifie que l'union des chunks de tous les joueurs est correcte.
#[test]
fn player_set_chunks_and_all_required() {
    let mut registry = PlayerRegistry::new();
    registry.add(1, "Alice".to_string());
    registry.add(2, "Bob".to_string());

    registry.set_player_chunks(1, [(0, 0, 0), (0, 0, 1)].into());
    registry.set_player_chunks(2, [(1, 0, 0), (0, 0, 0)].into());

    let required = registry.all_required_chunks();
    assert_eq!(required.len(), 3);
    assert!(required.contains(&(0, 0, 0)));
    assert!(required.contains(&(0, 0, 1)));
    assert!(required.contains(&(1, 0, 0)));
}

/// Vérifie que `all_required_chunks` est vide quand aucun joueur n'a de chunks.
#[test]
fn player_all_required_chunks_empty() {
    let registry = PlayerRegistry::new();
    assert!(registry.all_required_chunks().is_empty());
}

/// Vérifie que la suppression d'un joueur nettoie aussi ses chunks associés.
#[test]
fn player_remove_clears_chunks() {
    let mut registry = PlayerRegistry::new();
    registry.add(1, "Alice".to_string());
    registry.set_player_chunks(1, [(0, 0, 0)].into());
    registry.remove(&1);
    assert!(registry.all_required_chunks().is_empty());
}

// ---------------------------------------------------------------------------
// Section 2 : WorldState
// ---------------------------------------------------------------------------

/// Vérifie la lecture/écriture de la seed du monde.
#[test]
fn world_set_and_get_seed() {
    let mut world = WorldState::new();
    assert_eq!(world.get_seed(), 0);
    world.set_seed(12345);
    assert_eq!(world.get_seed(), 12345);
}

/// Vérifie que le voisinage 3×3×3 retourne exactement 27 chunks.
#[test]
fn world_get_required_chunks_count() {
    let chunks = WorldState::get_required_chunks(0, 0, 0);
    assert_eq!(chunks.len(), 27);
}

/// Vérifie que les 27 chunks du voisinage couvrent bien dx,dy,dz ∈ [-1, 1].
#[test]
fn world_get_required_chunks_coverage() {
    let chunks = WorldState::get_required_chunks(5, -3, 2);
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                assert!(chunks.contains(&(5 + dx, -3 + dy, 2 + dz)));
            }
        }
    }
}

/// Vérifie que modifier un bloc sur un chunk non généré ne panique pas
/// et n'ajoute pas de modification (simple avertissement dans les logs).
#[test]
fn world_set_block_non_generated() {
    let mut world = WorldState::new();
    world.set_block(10, 10, 10, 1);
    assert!(world.modifications.get_block_at(10, 10, 10).is_none());
}

/// Vérifie que retenir des chunks absents ne panique pas et laisse le monde vide.
#[test]
fn world_retain_chunks_empty() {
    let mut world = WorldState::new();
    world.set_seed(42);
    world.retain_chunks(&[(0, 0, 0)].into());
    assert!(world.world_generated_chunks.is_empty());
}

/// Vérifie la génération de chunks manquants.
#[test]
fn world_generate_missing() {
    let mut world = WorldState::new();
    world.set_seed(42);

    world.generate_missing(&[(0, 0, 0), (1, 0, 0)]);

    assert!(world.world_generated_chunks.contains_key(&(0, 0, 0)));
    assert!(world.world_generated_chunks.contains_key(&(1, 0, 0)));
}

/// Vérifie que générer plusieurs fois les mêmes chunks est idempotent.
#[test]
fn world_generate_missing_duplicates() {
    let mut world = WorldState::new();
    world.set_seed(42);

    world.generate_missing(&[(0, 0, 0)]);
    world.generate_missing(&[(0, 0, 0)]);
    assert_eq!(world.world_generated_chunks.len(), 1);
}

/// Vérifie la génération rectangulaire de chunks entre deux positions.
#[test]
fn world_generate_between_2_pos() {
    let mut world = WorldState::new();
    world.set_seed(42);

    let chunks = world.generate_chunks_between_2_pos(Point3::new(0, 0, 0), Point3::new(1, 1, 1));
    assert_eq!(chunks.len(), 8);
    for x in 0..=1 {
        for y in 0..=1 {
            for z in 0..=1 {
                assert!(chunks.contains_key(&(x, y, z)));
            }
        }
    }
}

/// Vérifie la génération sphérique : rayon 1 autour de l'origine donne 7 chunks.
#[test]
fn world_generate_in_radius() {
    let mut world = WorldState::new();
    world.set_seed(42);

    let chunks = world.generate_chunks_in_radius(Point3::new(0, 0, 0), 1);
    // dx² + dy² + dz² ≤ 1 → (0,0,0), (±1,0,0), (0,±1,0), (0,0,±1) = 7 chunks
    assert_eq!(chunks.len(), 7);
}

/// Vérifie que set_block fonctionne sur un chunk pré-généré.
#[test]
fn world_set_block_on_generated() {
    let mut world = WorldState::new();
    world.set_seed(42);

    world.generate_missing(&[(0, 0, 0)]);

    // Le chunk (0,0,0) couvre x:0..31, y:0..31, z:0..31
    world.set_block(5, 5, 5, 1);
    let modified = world.modifications.get_block_at(5, 5, 5);
    assert!(modified.is_some());
    assert_eq!(modified.unwrap().id, 1);
}

/// Vérifie que retain_chunks supprime les chunks non référencés.
#[test]
fn world_retain_chunks_removes_unused() {
    let mut world = WorldState::new();
    world.set_seed(42);

    world.generate_missing(&[(0, 0, 0), (1, 0, 0)]);
    assert_eq!(world.world_generated_chunks.len(), 2);

    world.retain_chunks(&[(0, 0, 0)].into());

    assert_eq!(world.world_generated_chunks.len(), 1);
    assert!(world.world_generated_chunks.contains_key(&(0, 0, 0)));
    assert!(!world.world_generated_chunks.contains_key(&(1, 0, 0)));
}

// ---------------------------------------------------------------------------
// Section 3 : AppState
// ---------------------------------------------------------------------------

/// Crée un état partagé initialisé avec une seed fixe pour les tests.
fn state_with_seed() -> AppState {
    let state = AppState::new();
    state.init_seed(42);
    state
}

/// Vérifie la lecture/écriture de la seed via AppState.
#[test]
fn state_init_seed_and_get_seed() {
    let state = AppState::new();
    assert_eq!(state.get_seed(), 0);
    state.init_seed(42);
    assert_eq!(state.get_seed(), 42);
}

/// Vérifie que init_random_seed s'exécute sans erreur.
#[test]
fn state_init_random_seed() {
    let state = AppState::new();
    state.init_random_seed();
}

/// Vérifie l'ajout d'un joueur dans l'état global.
#[test]
fn state_add_player() {
    let state = state_with_seed();
    state.add_player(1, "Alice".to_string());

    let players = state.get_all_players_vec().unwrap();
    assert_eq!(players.len(), 1);
    assert_eq!(players[0].id, 1);
    assert_eq!(players[0].username, "Alice");
}

/// Vérifie la suppression d'un joueur.
#[test]
fn state_remove_player() {
    let state = state_with_seed();
    state.add_player(1, "Alice".to_string());
    state.add_player(2, "Bob".to_string());
    state.remove_player(&1);

    let players = state.get_all_players_vec().unwrap();
    assert_eq!(players.len(), 1);
    assert_eq!(players[0].id, 2);
}

/// Vérifie que la suppression d'un joueur inexistant ne panique pas.
#[test]
fn state_remove_nonexistent() {
    let state = state_with_seed();
    state.add_player(1, "Alice".to_string());
    state.remove_player(&99);
    assert_eq!(state.get_all_players_vec().unwrap().len(), 1);
}

/// Vérifie que `get_all_players_vec` retourne `Some(vec![])` quand il n'y a personne.
#[test]
fn state_get_all_empty() {
    let state = state_with_seed();
    assert!(state.get_all_players_vec().unwrap().is_empty());
}

/// Vérifie que plusieurs joueurs sont bien listés.
#[test]
fn state_get_all_multiple() {
    let state = state_with_seed();
    state.add_player(1, "Alice".to_string());
    state.add_player(2, "Bob".to_string());
    state.add_player(3, "Charlie".to_string());
    assert_eq!(state.get_all_players_vec().unwrap().len(), 3);
}

/// Vérifie que la mise à jour de position ne panique pas et modifie bien l'état.
#[test]
fn state_update_player_position() {
    let state = state_with_seed();
    state.add_player(1, "Alice".to_string());

    state.update_player_position(
        1,
        Position {
            x: 35.0,
            y: 70.0,
            z: -30.0,
        },
        Rotation { x: 0.5, y: 0.3 },
    );

    let players = state.get_all_players_vec().unwrap();
    assert_eq!(players[0].position.x, 35.0);
    assert_eq!(players[0].position.y, 70.0);
    assert_eq!(players[0].position.z, -30.0);
}

/// Vérifie que set_block sur AppState ne panique pas (chunk non généré = warn logué).
#[test]
fn state_set_block() {
    let state = state_with_seed();
    state.set_block(10, 10, 10, 1);
}

// ---------------------------------------------------------------------------
// Section 4 : ProductionHandler (trait PacketHandler)
// ---------------------------------------------------------------------------

/// Configure un contexte de test avec un état et un canal broadcast.
fn handler_setup() -> (AppState, broadcast::Sender<Paquet>, broadcast::Receiver<Paquet>) {
    let state = AppState::new();
    state.init_seed(42);
    let (tx, rx) = broadcast::channel(512);
    (state, tx, rx)
}

/// Vérifie que `DonneesConnexion` est bien retourné tel quel par le handler.
#[test]
fn handler_donnees_connexion() {
    let (state, tx, _rx) = handler_setup();
    let ctx = HandlerContext {
        player_id: 1,
        state: &state,
        broadcaster: &tx,
    };

    let packet = create_handshake("Alice".to_string());
    let result = ProductionHandler.handle(packet, &ctx);

    assert!(result.is_some());
    let returned = result.unwrap();
    assert_eq!(returned.type_paquet, TypePaquet::Handshake);
    match &returned.contenu {
        ContenuPaquet::DonneesConnexion { username, .. } => assert_eq!(username, "Alice"),
        _ => panic!("type de paquet inattendu"),
    }
}

/// Vérifie que `PlayerTransformation` met à jour la position du joueur
/// et diffuse `MultiplePlayerTransformation` sur le canal broadcast.
#[test]
fn handler_player_transformation() {
    let (state, tx, mut rx) = handler_setup();
    state.add_player(1, "Alice".to_string());
    let ctx = HandlerContext {
        player_id: 1,
        state: &state,
        broadcaster: &tx,
    };

    let packet = create_player_update(1, 10.0, 20.0, 30.0, 0.5, 0.3);
    let result = ProductionHandler.handle(packet, &ctx);

    // Vérifie que le handler retourne le paquet original
    assert!(result.is_some());
    assert_eq!(result.unwrap().type_paquet, TypePaquet::PlayerTransformation);

    // Vérifie la mise à jour dans l'état partagé
    let players = state.get_all_players_vec().unwrap();
    assert_eq!(players[0].position.x, 10.0);

    // Vérifie que le broadcast a reçu la transformation groupée
    let broadcast_packet = rx.try_recv().unwrap();
    assert_eq!(broadcast_packet.type_paquet, TypePaquet::MultiplePlayerTransformation);
    match &broadcast_packet.contenu {
        ContenuPaquet::MultiplePlayerTransformation { data } => {
            assert_eq!(data.len(), 1);
            assert_eq!(data[0].player_id, 1);
            assert_eq!(data[0].position.x, 10.0);
        }
        _ => panic!("type de paquet broadcast inattendu"),
    }
}

/// Vérifie que `SetBlock` diffuse la modification sur le canal broadcast.
#[test]
fn handler_set_block() {
    let (state, tx, mut rx) = handler_setup();
    let ctx = HandlerContext {
        player_id: 1,
        state: &state,
        broadcaster: &tx,
    };

    let result = ProductionHandler.handle(new_set_block_paquet(10, 20, 30, 1), &ctx);
    assert!(result.is_some());

    // Vérifie que le broadcast a bien propagé le SetBlock
    let broadcast_packet = rx.try_recv().unwrap();
    assert_eq!(broadcast_packet.type_paquet, TypePaquet::SetBlock);
    match &broadcast_packet.contenu {
        ContenuPaquet::SetBlock { x, y, z, block_id } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 20);
            assert_eq!(*z, 30);
            assert_eq!(*block_id, 1);
        }
        _ => panic!("type de paquet broadcast inattendu"),
    }
}

/// Vérifie que `Ping` retourne un `Pong` avec le même timestamp.
#[test]
fn handler_ping_returns_pong() {
    let (state, tx, _rx) = handler_setup();
    let ctx = HandlerContext {
        player_id: 1,
        state: &state,
        broadcaster: &tx,
    };

    let result = ProductionHandler.handle(new_ping_paquet(1000), &ctx);
    assert!(result.is_some());

    let returned = result.unwrap();
    assert_eq!(returned.type_paquet, TypePaquet::Pong);
    match &returned.contenu {
        ContenuPaquet::Pong { timestamp } => assert_eq!(*timestamp, 1000),
        _ => panic!("type de paquet inattendu"),
    }
}

/// Vérifie que `Pong` est retourné tel quel.
#[test]
fn handler_pong_returns_packet() {
    let (state, tx, _rx) = handler_setup();
    let ctx = HandlerContext {
        player_id: 1,
        state: &state,
        broadcaster: &tx,
    };

    let packet = new_pong_paquet(2000);
    let result = ProductionHandler.handle(packet, &ctx);

    assert!(result.is_some());
    assert_eq!(result.unwrap().type_paquet, TypePaquet::Pong);
}

/// Vérifie qu'un paquet non géré (ex: WorldData) retourne None (éjection du client).
#[test]
fn handler_unhandled_returns_none() {
    let (state, tx, _rx) = handler_setup();
    let ctx = HandlerContext {
        player_id: 1,
        state: &state,
        broadcaster: &tx,
    };

    let result = ProductionHandler.handle(
        Paquet::new(TypePaquet::WorldData, ContenuPaquet::DonneesMonde { chunks: vec![] }),
        &ctx,
    );
    assert!(result.is_none());
}
