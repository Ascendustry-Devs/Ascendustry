//! Types et files de requêtes de maillage (mesh).
//!
//! Ce module regroupe tout ce qui circule entre le [`World`](crate::world::world::World)
//! (qui produit les requêtes) et le [`WorldMesh`](crate::render::meshing::world::WorldMesh)
//! (qui les consomme).

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use game::world::chunk_position::ChunkPosition;
use game::world::data::chunk::Chunk;
use game::world::scheduling::chunk_priority_sq;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

// TODO : rajouter les 8 coins extérieurs du chunk (+x;+y;+z, +x;+y;-z, etc...) et les 12 voisins diagonaux (+x;+y;z, +x;-y;z, etc...) puis adapter le calcul des plans (PaddedPlane) pour régler le bug de l'AO aux bordures des chunks qui a lieu seulement sur les lignes de blocs ayant 2 chunks voisins minimum.

/// Instantané d'un chunk et de ses 6 voisins directs.
///
/// Les voisins sont nécessaires au maillage "greedy" : sans eux, les faces
/// situées sur les frontières du chunk seraient générées (faces intérieures
/// inutiles) ou manquantes (trous entre deux chunks).
#[derive(Clone, PartialEq, Eq)]
pub struct MeshSnapshot {
    pub main: Arc<Chunk>,
    pub neg_x: Option<Arc<Chunk>>,
    pub pos_x: Option<Arc<Chunk>>,
    pub neg_y: Option<Arc<Chunk>>,
    pub pos_y: Option<Arc<Chunk>>,
    pub neg_z: Option<Arc<Chunk>>,
    pub pos_z: Option<Arc<Chunk>>,
}

/// Réponse du monde de maillage : les coordonnées du chunk désormais maillé.
pub type MeshResponse = ChunkPosition;

/// Identifiant d'un chunk à supprimer du monde de maillage.
pub type MeshRequestDelete = ChunkPosition;

/// Demande de maillage d'un chunk, accompagnée de l'instantané nécessaire.
#[derive(Clone)]
pub struct MeshRequestAdd {
    pub coords: ChunkPosition,
    pub snapshot: MeshSnapshot,
}

impl MeshRequestAdd {
    pub const fn new(coords: ChunkPosition, snapshot: MeshSnapshot) -> Self {
        Self { coords, snapshot }
    }
}

impl PartialEq for MeshRequestAdd {
    fn eq(&self, other: &Self) -> bool {
        self.coords == other.coords
    }
}
impl Eq for MeshRequestAdd {}
impl Hash for MeshRequestAdd {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.coords.hash(state);
    }
}

/// File **ordonnée** de requêtes de maillage, indexée par coordonnées de chunk.
///
/// Garanties :
/// - l'ordre d'insertion est conservé ;
/// - ré-insérer un chunk déjà présent **remplace** son instantané tout en
///   conservant sa position dans la file (une modification de bloc sur un
///   chunk déjà en attente ne doit pas être perdue, mais ne doit pas non
///   plus désorganiser la file) ;
/// - la file peut être triée par priorité (distance au joueur) à tout moment,
///   y compris juste avant la soumission au pool de maillage.
///
/// L'implémentation combine un `Vec` (ordre) et une table de hachage
/// (accès O(1) et remplacement du snapshot).
#[derive(Default)]
pub struct MeshRequestQueue {
    /// Coordonnées des chunks en attente, dans l'ordre de la file.
    order: Vec<ChunkPosition>,
    /// Instantané courant associé à chaque chunk en attente.
    requests: FxHashMap<ChunkPosition, MeshRequestAdd>,
}

impl MeshRequestQueue {
    pub fn new() -> Self {
        Self {
            order: Vec::new(),
            requests: FxHashMap::with_hasher(FxBuildHasher),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            order: Vec::with_capacity(capacity),
            requests: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
        }
    }

    /// Insère une requête.
    ///
    /// - `true` : le chunk n'était pas encore en attente, il a été ajouté.
    /// - `false` : le chunk était déjà en attente, son instantané a été
    ///   **remplacé** par celui fourni (plus récent) et sa position conservée.
    pub fn insert(&mut self, request: MeshRequestAdd) -> bool {
        match self.requests.get_mut(&request.coords) {
            Some(existing) => {
                *existing = request;
                false
            }
            None => {
                self.order.push(request.coords);
                self.requests.insert(request.coords, request);
                true
            }
        }
    }

    /// Requête située à la position `index` dans la file.
    pub fn get(&self, index: usize) -> Option<&MeshRequestAdd> {
        let coords = self.order.get(index)?;
        self.requests.get(coords)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Capacité allouée pour la liste d'ordre.
    pub fn capacity(&self) -> usize {
        self.order.capacity()
    }

    /// Capacité allouée pour la table des requêtes.
    pub fn requests_capacity(&self) -> usize {
        self.requests.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn contains(&self, coords: &ChunkPosition) -> bool {
        self.requests.contains_key(coords)
    }

    /// Retire une requête de la file. Renvoie la requête retirée, si elle existait.
    pub fn remove(&mut self, coords: &ChunkPosition) -> Option<MeshRequestAdd> {
        let request = self.requests.remove(coords)?;
        self.order.retain(|c| c != coords);
        Some(request)
    }

    /// Vide la file et renvoie les requêtes **dans l'ordre de la file**.
    pub fn drain(&mut self) -> Vec<MeshRequestAdd> {
        let mut drained = Vec::with_capacity(self.order.len());
        for coords in self.order.drain(..) {
            if let Some(request) = self.requests.remove(&coords) {
                drained.push(request);
            }
        }
        drained
    }

    /// Ne conserve que les requêtes pour lesquelles `predicate` renvoie `true`.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&MeshRequestAdd) -> bool,
    {
        self.order.retain(|coords| {
            let keep = self.requests.get(coords).is_some_and(&mut predicate);
            if !keep {
                self.requests.remove(coords);
            }
            keep
        });
    }

    /// Trie la file par priorité croissante : les chunks les plus proches du
    /// joueur sont servies en premier.
    ///
    /// Le tri est **stable**, donc deux chunks de même priorité conservent
    /// leur ordre d'insertion : le comportement reste déterministe.
    pub fn sort_by_priority(&mut self, player_chunk: &ChunkPosition) {
        self.order.sort_by_key(|coords| chunk_priority_sq(coords, player_chunk));
    }

    pub fn clear(&mut self) {
        self.order.clear();
        self.requests.clear();
    }
}

/// Lot de requêtes de maillage transmis du monde au monde de maillage.
pub struct MeshRequestMessage {
    /// Requêtes d'ajout / de remeshing, dans l'ordre de la file.
    pub add: MeshRequestQueue,
    /// Chunks dont le maillage doit être supprimé.
    pub delete: FxHashSet<MeshRequestDelete>,
}

impl MeshRequestMessage {
    pub fn empty() -> Self {
        Self {
            add: MeshRequestQueue::new(),
            delete: FxHashSet::with_hasher(FxBuildHasher),
        }
    }
}

impl Default for MeshRequestMessage {
    fn default() -> Self {
        Self::empty()
    }
}
