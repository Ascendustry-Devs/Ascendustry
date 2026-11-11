use game::{types::Position, world::chunk_position::ChunkPosition};

use client::player::{
    controllers::{spectator::FreeCameraController, walk::WalkPlayerController},
    PlayerState,
};

fn new_state(spawn_pos: Position) -> PlayerState {
    PlayerState::new(
        0,
        Box::new(FreeCameraController::new(0.00390625)),
        Box::new(WalkPlayerController),
        spawn_pos,
    )
}

/// Reproduit le test d'appartenance utilisé par l'élagage de rendu.
fn is_in_render_range(range: [i32; 6], chunk: &ChunkPosition) -> bool {
    let (x, y, z) = chunk.into();
    x >= range[0] && x <= range[1] && y >= range[2] && y <= range[3] && z >= range[4] && z <= range[5]
}

#[test]
fn test_render_range_contains_player_chunk_on_spawn() {
    // Régression : la boîte de rendu doit contenir le chunk du joueur dès
    // la première frame. Avant, un cache mal initialisé éliminait le monde
    // entier de l'écran jusqu'au premier changement de chunk du joueur.
    let state = new_state(Position::new(0.5, 12.0, 0.5));
    let range = state.get_rendered_chunk_range();
    let cpos = state.cpos.current();

    assert!(
        is_in_render_range(range, cpos),
        "le chunk du joueur doit appartenir à la boîte de rendu, sinon rien n'est affiché"
    );
}

#[test]
fn test_render_range_follows_teleportation() {
    // Régression : après une téléportation, la boîte de rendu doit suivre la
    // nouvelle position, y compris si le joueur change de chunk.
    let mut state = new_state(Position::new(0.5, 12.0, 0.5));
    state.set_pos(Position::new(100.5, 12.0, -64.5));

    let cpos = state.cpos.current();
    assert_eq!(
        *cpos,
        ChunkPosition::new(3, 0, -3),
        "la position en chunk doit suivre la téléportation"
    );

    let range = state.get_rendered_chunk_range();
    assert!(
        is_in_render_range(range, cpos),
        "la boîte de rendu doit contenir le chunk de destination"
    );
}

#[test]
fn test_render_range_stays_consistent_when_chunk_does_not_change() {
    // Se déplacer à l'intérieur du même chunk ne doit pas modifier la
    // boîte de rendu.
    let mut state = new_state(Position::new(0.5, 12.0, 0.5));
    let before = state.get_rendered_chunk_range();

    state.set_pos(Position::new(5.5, 13.0, 3.5));
    assert_eq!(state.get_rendered_chunk_range(), before);
}
