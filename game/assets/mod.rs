pub mod block_loader;
pub mod item_loader;

use std::path::{Path, PathBuf};

/// Racine des assets lors d'une installation du jeu (assets shippés à côté
/// du binaire).
const RUNTIME_ASSETS_DIR: &str = "assets";

/// Racine des assets en développement et lors des tests.
///
/// Résolue à la compilation : le répertoire courant n'est pas la racine du
/// workspace lorsqu'on lance `cargo test` depuis le crate `game`.
const DEV_ASSETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets");

/// Résout le chemin d'un asset, relatif à la racine des assets.
///
/// Deux racines sont essayées, dans l'ordre :
/// - celle du workspace (développement, tests) ;
/// - celle du répertoire courant (installation du jeu).
///
/// Si aucune des deux n'existe, le chemin relatif au répertoire courant est
/// retourné : l'erreur produite par l'appelant reste alors explicite.
pub fn resolve_asset_path(relative: &str) -> PathBuf {
    let dev_path = Path::new(DEV_ASSETS_DIR).join(relative);
    if dev_path.exists() {
        return dev_path;
    }

    Path::new(RUNTIME_ASSETS_DIR).join(relative)
}
