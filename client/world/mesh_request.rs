//! Types et files de requêtes de maillage (mesh).
//!
//! Ce module regroupe tout ce qui circule entre le [`World`](crate::world::world::World)
//! (qui produit les requêtes) et le [`WorldMesh`](crate::render::meshing::world::WorldMesh)
//! (qui les consomme).

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use game::world::data::chunk::Chunk;
use game::world::scheduling::chunk_priority_sq;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

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
pub type MeshResponse = (i32, i32, i32);

/// Identifiant d'un chunk à supprimer du monde de maillage.
pub type MeshRequestDelete = (i32, i32, i32);

/// Demande de maillage d'un chunk, accompagnée de l'instantané nécessaire.
#[derive(Clone)]
pub struct MeshRequestAdd {
    pub coords: (i32, i32, i32),
    pub snapshot: MeshSnapshot,
}

impl MeshRequestAdd {
    pub const fn new(coords: (i32, i32, i32), snapshot: MeshSnapshot) -> Self {
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
    order: Vec<(i32, i32, i32)>,
    /// Instantané courant associé à chaque chunk en attente.
    requests: FxHashMap<(i32, i32, i32), MeshRequestAdd>,
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

    pub fn contains(&self, coords: &(i32, i32, i32)) -> bool {
        self.requests.contains_key(coords)
    }

    /// Retire une requête de la file. Renvoie la requête retirée, si elle existait.
    pub fn remove(&mut self, coords: &(i32, i32, i32)) -> Option<MeshRequestAdd> {
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
    pub fn sort_by_priority(&mut self, player_chunk: (i32, i32, i32)) {
        self.order.sort_by_key(|coords| chunk_priority_sq(*coords, player_chunk));
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

#[cfg(test)]
mod tests {
    use super::*;
    use game::world::data::block::BlockInstance;

    /// Construit un instantané identifiable : le premier bloc du chunk
    /// principal porte l'identifiant `marker`.
    fn make_snapshot(marker: u32) -> MeshSnapshot {
        let main = Arc::new(Chunk {
            blocks: vec![BlockInstance::new(marker); 32 * 32 * 32],
            x: 0,
            y: 0,
            z: 0,
        });
        MeshSnapshot {
            main: Arc::clone(&main),
            neg_x: None,
            pos_x: None,
            neg_y: None,
            pos_y: None,
            neg_z: None,
            pos_z: None,
        }
    }

    /// Identifiant porté par le premier bloc de l'instantané.
    fn marker_of(snapshot: &MeshSnapshot) -> u32 {
        snapshot.main.blocks[0].get_block_id()
    }

    #[test]
    fn test_queue_preserves_insertion_order() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((2, 0, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(2)));
        queue.insert(MeshRequestAdd::new((1, 0, 0), make_snapshot(3)));

        let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
        assert_eq!(coords, vec![(2, 0, 0), (0, 0, 0), (1, 0, 0)]);
    }

    #[test]
    fn test_queue_upsert_replaces_snapshot_and_keeps_position() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((1, 0, 0), make_snapshot(2)));

        // Ré-insertion du premier chunk avec un instantané plus récent.
        let inserted = queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(99)));
        assert!(!inserted, "le chunk était déjà présent");
        assert_eq!(queue.len(), 2, "aucun doublon ne doit être créé");

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].coords, (0, 0, 0), "la position doit être conservée");
        // L'instantané doit avoir été remplacé.
        assert_eq!(marker_of(&drained[0].snapshot), 99);
    }

    #[test]
    fn test_queue_sort_by_priority() {
        let mut queue = MeshRequestQueue::new();
        // Insertion volontairement dans le désordre.
        queue.insert(MeshRequestAdd::new((5, 0, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((1, 0, 0), make_snapshot(2)));
        queue.insert(MeshRequestAdd::new((3, 0, 0), make_snapshot(3)));
        queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(4)));

        queue.sort_by_priority((0, 0, 0));

        let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
        assert_eq!(coords, vec![(0, 0, 0), (1, 0, 0), (3, 0, 0), (5, 0, 0)]);
    }

    #[test]
    fn test_queue_sort_by_priority_keeps_vertical_discount() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((0, 4, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((4, 0, 0), make_snapshot(2)));

        queue.sort_by_priority((0, 0, 0));

        let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
        // 4 chunks en vertical (priorité 8) < 4 chunks à l'horizontale (16).
        assert_eq!(coords, vec![(0, 4, 0), (4, 0, 0)]);
    }

    #[test]
    fn test_queue_sort_by_priority_around_the_origin() {
        // Le joueur est à l'origine : les chunks sont répartis de part et
        // d'autre. L'ordre doit être correct, quel que soit le signe des
        // coordonnées (les distances sont signées, pas "unsigned").
        let mut queue = MeshRequestQueue::new();
        for coords in [(3, 0, 0), (-3, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 0)] {
            queue.insert(MeshRequestAdd::new(coords, make_snapshot(1)));
        }

        queue.sort_by_priority((0, 0, 0));

        let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
        // Distances : 0, puis 1 (1 et -1), puis 9 (3 et -3).
        // Le tri est stable : à distance égale, l'ordre d'insertion est gardé.
        assert_eq!(coords, vec![(0, 0, 0), (1, 0, 0), (-1, 0, 0), (3, 0, 0), (-3, 0, 0)]);
    }

    #[test]
    fn test_queue_sort_is_stable() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((1, 0, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((1, 0, 1), make_snapshot(2)));
        queue.insert(MeshRequestAdd::new((-1, 0, 0), make_snapshot(3)));

        queue.sort_by_priority((0, 0, 0));

        let coords: Vec<_> = queue.drain().iter().map(|r| r.coords).collect();
        // (1, 0, 0) et (-1, 0, 0) sont à distance 1 : le tri étant stable,
        // leur ordre d'insertion est conservé. (1, 0, 1) est à distance 2.
        assert_eq!(coords, vec![(1, 0, 0), (-1, 0, 0), (1, 0, 1)]);
    }

    #[test]
    fn test_queue_remove_and_contains() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((1, 0, 0), make_snapshot(2)));

        assert!(queue.contains(&(0, 0, 0)));
        assert!(queue.remove(&(0, 0, 0)).is_some());
        assert!(!queue.contains(&(0, 0, 0)));
        assert_eq!(queue.len(), 1);
        assert!(queue.remove(&(9, 9, 9)).is_none());
    }

    #[test]
    fn test_queue_retain() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(1)));
        queue.insert(MeshRequestAdd::new((1, 0, 0), make_snapshot(2)));

        queue.retain(|request| request.coords.0 == 1);

        assert_eq!(queue.len(), 1);
        assert!(queue.contains(&(1, 0, 0)));
        assert!(!queue.contains(&(0, 0, 0)));
    }

    #[test]
    fn test_queue_drain_empties_the_queue() {
        let mut queue = MeshRequestQueue::new();
        queue.insert(MeshRequestAdd::new((0, 0, 0), make_snapshot(1)));
        assert_eq!(queue.drain().len(), 1);
        assert!(queue.is_empty());
        assert_eq!(queue.drain().len(), 0);
    }
}
