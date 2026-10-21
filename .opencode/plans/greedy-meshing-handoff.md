# Greedy Meshing — Handoff (état commité `44334a4`)

> Document de passation. Objectif : permettre à un autre poste (et/ou un assistant) de comprendre
> **ce qui a changé**, **comment le système actuel fonctionne**, **les bugs corrigés**, et
> **les objectifs futurs** (profiling + optimisation < 100 µs/chunk).
>
> Commit de référence : `44334a4` — « Greedy Mesher optimisation (~t/2) grâce à de la vectorisation… ».
> Version précédente (lente, ~780 µs/chunk) : `ef82588` — « Ajout de l'eau ».

---

## 0. Résumé exécutif

L'ancien greedy meshing (`client/render/meshing/chunk.rs`, basé sur `&[bool; 39304]`, cf. HEAD `ef82588`)
mettait **~780 µs/chunk**. La nouvelle implémentation (`client/render/meshing/chunk_new.rs`) est une
**réécriture complète en bit-packing + SIMD** qui produit une sortie **strictement identique bit à bit**
(validée par tests) pour **~403 µs/chunk en jeu** (mesuré sur 831 chunks), soit ≈ ÷2.

Changements majeurs :
1. Solidité **bit-packed** (`u32`/`u64`) au lieu d'un tableau de 39 304 `bool`.
2. **SIMD** AVX2/SSE2 pour la construction du cube de solidité et la fusion ; fallback scalaire.
3. **Un seul chemin générique** pour les 3 axes (au lieu de `make_greedy_x/y/z` dupliqués ×3).
4. **Correction de fuites mémoire** (`BufferPool` non borné, copies d'ids).
5. **Correction d'un bug de placement** : les chunks n'étaient pas positionnés dans le monde.

---

## 1. Contexte / fichiers

| Fichier | Rôle |
|---|---|
| `client/render/meshing/chunk_new.rs` | **Nouvelle implémentation** : solidité bit + maillage générique + tests. |
| `client/render/meshing/chunk.rs` | Ancienne implémentation (`ChunkMesh`, `PaddedChunkSliceMask`). Conservée pour le rendu GPU (`ChunkMesh::update`) et comme référence de test. |
| `client/render/meshing/processor.rs` | Worker `GreedyMeshingProcessor` — appelle `chunk_new::make_greedy_chunk`. |
| `client/render/meshing/world.rs` | Orchestration : soumission, `BufferPool`, suivi des meshes. |
| `project_core/buffer_pool.rs` | Pool de buffers `Vec<Vertex>` (borné désormais). |
| `client/render/utils/padded_chunk.rs` | `PaddedChunk` (157 Kio, `from_snapshot`) — utilisé **uniquement par la référence de test**, plus dans le chemin critique. |
| `client/render/utils/face_mask.rs` | `FaceMask` packé — utilisé par l'ancienne implémentation et la référence de test. |
| `client/world/world.rs` | `MeshSnapshot` (main + 6 voisins), génération/submission des snapshots. |

Chaîne d'appel en production :

```
world.rs (MeshSnapshot + coords)  →  WorldMesh::submit_meshes
   →  WorkerPool<GreedyMeshingProcessor>  →  processor.rs::process
       →  chunk_new::make_greedy_chunk(snapshot, &mut vertices, origin, texture_lookup)
   →  WorldMesh::compute_generated_meshes  →  ChunkMesh::update (upload GPU)
```

---

## 2. Ce qui a changé (diff résumé)

### 2.1 `game/Cargo.toml`, `game/world/data/block.rs`
- `BlockInstance` dérive `Pod, Zeroable` (bytemuck) afin de pouvoir lire `Vec<BlockInstance>` comme
  `&[u32]` **sans copie** (`bytemuck::cast_slice`). `BlockInstance` est `#[repr(C)]` avec un unique
  champ `id: u32`, donc la vue `&[u32]` est valide.
- `bytemuck = { workspace = true }` ajouté.

### 2.2 `project_core/buffer_pool.rs` — **fuite mémoire corrigée**
Avant : le pool gardait un nombre **illimité** de buffers, chacun de capacité initiale `256 K` `Vertex`
= 8 Mio. Chaque buffer rendu y était poussé sans jamais être réduit ⇒ **croissance cumulée → OOM**.

Après :
- champ `max_buffers` (défaut 16), méthode `with_max_buffers(capacity, max_buffers)` ;
- rejet des buffers dont `capacity() > MAX_KEPT_CAPACITY` (256 K éléments) ;
- `lock_buffers()` tolère un mutex empoisonné (`into_inner`) au lieu de paniquer.

### 2.3 `client/render/meshing/world.rs`
- `BufferPool::new(1024*256)` → `BufferPool::with_max_buffers(1024*256, worker_count)`.

### 2.4 `client/render/meshing/processor.rs` — **bug de placement corrigé**
Avant (mon branchement intermédiaire) : `make_greedy_chunk(..., [cx as f32, cy as f32, cz as f32], ...)`
→ tous les chunks étaient rendus **empilés à l'origine** (faces superposées qui flickent).

Après : origine monde correcte
```rust
let origin = [
    (cx * CHUNK_SIZE) as f32,
    (cy * CHUNK_SIZE) as f32,
    (cz * CHUNK_SIZE) as f32,
];
```
(La référence HEAD faisait bien `cwx = cx * CHUNK_SIZE`.)

### 2.5 `Cargo.toml`
- Profils dev/release ajustés (`opt-level`, `codegen-units`), ajout d'un profil `ultra_release`
  (lto, codegen-units 1, overflow-checks off).

### 2.6 `client/game.rs` / `client/main.rs`
- Instrumentation de mesure in-game (overlay **Ctrl+O**) : `TIME_SPENT_MESHING_NANOS`,
  `TIME_SPENT_PREPARING_NANOS`, `CHUNKS_MESHED` (statiques mutables). Affiche le temps moyen
  par chunk (ns/µs/ms). Sert au profiling manuel.
- `FPS_CAP` ramené à 60 pour les tests.

---

## 3. Comment fonctionne le nouveau mesher (`chunk_new.rs`)

### 3.1 Constantes et conventions

```rust
const CHUNK: usize = 32;
const PAD:   usize = 34;          // plans paddés : lignes/colonnes 0..=33
const AO_TABLE: [u8; 8] = [3, 2, 2, 0, 2, 1, 1, 0];  // idx = (corner<<2)|(side1<<1)|side2
```

Indices de voisins : `NEG_X=0, POS_X=1, NEG_Y=2, POS_Y=3, NEG_Z=4, POS_Z=5`.

Conventions d'axes :
- **X** : plan = x, lignes = y, colonnes = z. Face +/− : `Right`/`Left`.
- **Y** : plan = y, lignes = z, colonnes = x. Face +/− : `Top`/`Bottom`.
- **Z** : plan = z, lignes = x, colonnes = y. Face +/− : `Front`/`Back`.

### 3.2 Géométrie AO (validée)

- Les 3 échantillons AO d'une face sont **toujours dans le plan du côté air** :
  - face `+axe` (bloc solide au plan `d`, face vers `d+1`) → AO sur le plan `d+1` ;
  - face `−axe` (bloc solide au plan `d+1`, face vers `d`) → AO sur le plan `d`.
- Donc 33 plans utiles par axe (`0..=32`) ; plan 33 jamais échantillonné.
- Plans paddés de **34 lignes × 34 bits**, stockés en `[u64; 34]` (1 mot par ligne ⇒ jamais de
  débordement de shift). Les bords du padding sont de l'air, exactement comme `PaddedChunk`.
- `ao_neighbors(face, corner)` est la copie **fidèle** de l'ancien `ChunkMesh::get_ao_neighbors`
  (inversion `(u,v)` sauf Front/Back conservée), et `ao_lateral_offsets` en dérive les décalages
  `(delta_ligne, delta_colonne)` par coin.

### 3.3 Structures

```rust
struct BitMatrix { rows: [u32; CHUNK] }          // 1 ligne = 32 bits

struct NeighborLayers { primary: BitMatrix, secondary: BitMatrix }
// La couche de surface d'un voisin, dans ses deux orientations (primary + transposée).

struct PaddedPlane { rows: [u64; PAD] }         // plan de solidité paddé

struct ChunkSolidity {
    main_chunk:   Arc<Chunk>,                    // pour lire les ids à la demande (pas de copie)
    core:   [u32; CHUNK*CHUNK],                  // core[z*32+y]  = bits sur x
    core_t: [u32; CHUNK*CHUNK],                  // core_t[z*32+x]= bits sur y
    core_x: [u32; CHUNK*CHUNK],                  // core_x[x*32+y]= bits sur z
    neighbors: [NeighborLayers; 6],
    neighbor_chunks: [Option<Arc<Chunk>>; 6],    // ids des voisins lus à la demande
}
```

- `core` est le cube de solidité du chunk principal (x-contigu, layout `x + 32y + 1024z`).
- `core_t` / `core_x` sont obtenus par `transpose32` (voir §3.6).
- Les voisins sont représentés par **2 matrices de bits** (`primary`, `secondary`), ce qui suffit car
  seul un bord de 1 case de chaque voisin est nécessaire.

### 3.4 Construction (`ChunkSolidity::new`)

1. `main_ids = bytemuck::cast_slice(&snapshot.main.blocks)` (vue `&[u32]`, **sans copie**).
2. `build_core(main_ids, &mut core)` — voir §3.5 (SIMD).
3. `core_t` : pour chaque `z`, transposer la tranche 32×32.
4. `core_x` : reconstruit par double transposition.
5. Pour chaque voisin présent : `build_neighbor_layers(chunk, kind)` → `primary` (bits sur l'axe
   tangent) puis `secondary = transpose32(primary)`.

### 3.5 `build_core` (SIMD)

Construit `out[z*32 + y]` = mot de 32 bits où le bit `x` vaut 1 si `blocks[x + 32y + 1024z].id != 0`.

- **x86_64 + AVX2** : `build_core_avx2` — 8 ids par itération, `_mm256_cmpeq_epi32` contre zéro.
- **x86_64 sans AVX2** : `build_core_sse2` — 4 ids par itération.
- **autres cibles** : `build_core_scalar` (`#[allow(dead_code)]`, utilisé hors x86_64).
- Détection : `has_avx2()` via `std::arch::is_x86_feature_detected!("avx2")`.

### 3.6 `transpose32`

⚠️ **Remarque importante** : l'ancienne version « Hacker's Delight » (delta swaps) embrouillait le sens
des bits et ne correspondait pas au résultat attendu. Elle a été remplacée par une transposition
**naïve** (32×32, double boucle), évidemment correcte, testée par `transpose_is_correct`. Le coût est
négligeable (≤ 64 tranches). Si on veut re-SIMD ici plus tard, il faudra re-valider contre la version
naïve (le test existe).

### 3.7 `build_plane` / maillage

Pour un axe, `mesh_axis` parcourt les plans de face `d ∈ 0..32` :

```
lo = plan d       (plan 0 = couche du voisin négatif)
hi = plan d+1     (pour d<31 : tranche x=d du chunk)
lo_core = (lo.rows[r+1] >> 1) as u32     // on retire le padding (colonnes 1..=32)
hi_core = (hi.rows[r+1] >> 1) as u32
faces   = lo_core ^ hi_core              // cellules où solidité change
```

Pour chaque bit de face `c` :
- si `hi_core` bit `c` = 1 (solide au plan `d+1` = tranche `x=d`) :
  → face **négative**, AO sur `lo`, id = `solid_block_id(axis, d, r, c)`.
- sinon (solide au plan `d`) :
  → face **positive**, AO sur `hi`,
  id = `neighbor_block_id(axis, r, c)` si `d==0` sinon `solid_block_id(axis, d-1, r, c)`.

> ⚠️ **Bug historique corrigé** : ce choix d'id était inversé (la branche « hi solide » envoyait sur le
> voisin). C'est ce qui faisait échouer les tests de texture.

AO : `face_ao(offsets, plane, u, v)` lit 3 bits par coin depuis `plane` et applique `AO_TABLE`.
Empaquetage : `v0<<6 | v1<<4 | v2<<2 | v3` (coins `[BL, BR, TL, TR]`).

Clé de fusion `u64` : `(texture << 16) | (dir << 8) | packed_ao`.

### 3.8 Fusion greedy

Pour chaque ligne `u`, parcours des colonnes `v` :
- `present[u]` = bitmap des cellules portant une face non encore visitée.
- Extension en largeur (lignes `r = u+1..`) : `match_row(r, keys, present[r], key)` → bitmap des
  colonnes de la ligne `r` dont la clé est identique. On s'arrête à la 1ʳᵉ ligne dont la cellule `v`
  ne matche pas.
- `common = match_row(u,...) & (!0u32 << v)` puis `common &= matched` pour chaque ligne retenue.
- Hauteur : `height = (!(common >> v)).trailing_zeros()` (**le `& (!0<<v)` est essentiel** : sinon
  les colonnes déjà fusionnées `< v` faussent la hauteur et provoquent des boucles infinies).
- Émission d'un quad `width × height`, puis on efface les bits `[v, v+height)` de chaque ligne.

> **Bug historique corrigé** : `common` n'était pas masqué `& (!0<<v)`, ce qui pouvait donner
> `height = 0` → **infinite loop** → OOM (c'était une des causes de tes OOM).

### 3.9 `match_row` (AVX2)

- `match_row_scalar` : itère sur les bits de `present` et compare `keys[base+c] == key`.
- `match_row_avx2` : compare 32 clés u64 en 8 itérations de 4 lanes.

> ⚠️ **Bug historique corrigé** (`match_row_avx2`) : `_mm256_movemask_pd` ne renvoie que **4 bits**
> (un par lane de 64 bits) ; le code avançait de 8 (`i += 8`) et écrasait les résultats ⇒
> comparaisons manquées ⇒ faces ratées / `height=0` / OOM. Correction : `i += 4`, masque de lanes
> utilisé directement, et intersection `& present` **au niveau de l'appelant** (contrat aligné avec
> la version scalaire).

### 3.10 Émission (`mesh_axis`, suite)

- Positions des 4 sommets selon l'axe (cf. `offsets`/`uvs` dans le code), origine =
  `origin + d*main + u*row + v*col`.
- `reverse` : X → `dir.is_negative()` ; Y/Z → `dir.is_positive()`.
- Triangles : si `reverse` `[v0,v1,v2,v2,v1,v3]` sinon `[v1,v0,v3,v3,v0,v2]`.

### 3.11 Point d'entrée

```rust
pub fn make_greedy_chunk(
    snapshot: &MeshSnapshot,
    vertices: &mut Vec<Vertex>,
    chunk_origin: [f32; 3],
    texture_lookup: &[u32],
)
```
Construit `ChunkSolidity` puis appelle `mesh_axis` pour X, Y, Z **dans cet ordre** (ordre des triangles
identique à la référence ⇒ égalité bit-à-bit).

---

## 4. Validation (tests)

Dans `chunk_new.rs`, module `#[cfg(test)] mod tests` :

- `struct Reference` : **copie fidèle** de l'ancienne implémentation (`chunk.rs` HEAD `ef82588`,
  version `&[bool; PADDED_CHUNK_BLOCK_CBE_USIZE]` + `PaddedChunk` + `FaceMask`). Sert de référence
  bit-à-bit. ⚠️ Ne pas utiliser la version du working tree de `chunk.rs` : son `PaddedChunkSliceMask`
  est **cassé** (34 bits dans un `u32` → shift overflow, round-trip 3616/4096 échecs).
- `check(snapshot, label)` : compare `Vec<Vertex>` de la référence et de `make_greedy_chunk` via
  `bytemuck::cast_slice::<Vertex, u8>` (égalité **octet à octet**).

Tests :
| Test | Scénario |
|---|---|
| `transpose_is_correct` | transposition vs force brute |
| `empty_chunk_matches` | chunk vide |
| `solid_chunk_matches` | chunk plein |
| `checkerboard_matches` | damier 3D |
| `random_chunks_match` | 8 densités, voisins aléatoires présents/absents |
| `terrain_matches` | génération type terrain |

Commande validée (⚠️ **single-thread** pour borner la mémoire des tests lourds) :
```bash
cargo test -p client --release --lib chunk_new -- --test-threads=1
# → 6 passed ; ~0.20 s en release, ~1.5 s en debug
```

Résultats mesurés in-game (overlay Ctrl+O) :
- ancien (`ef82588`) : ~780 µs/chunk
- nouveau (`44334a4`) : **~403 µs/chunk** sur 831 chunks (÷ ≈ 2)

---

## 5. Bugs corrigés (historique — utile pour ne pas les réintroduire)

| # | Bug | Symptôme | Correction |
|---|---|---|---|
| 1 | `BufferPool` non borné | RAM qui croît jusqu'à l'OOM | `max_buffers` + rejet des buffers géants |
| 2 | Copies d'ids dans `ChunkSolidity` | working set ~900 Kio/chunk | lire `Arc<Chunk>.blocks[i].id` à la demande |
| 3 | `match_row_avx2` (movemask 4 bits / pas de 8) | faces ratées, `height=0`, **OOM** | `i += 4`, masque de lanes direct, `& present` côté appelant |
| 4 | `common` non masqué `& (!0<<v)` | boucle infinie, OOM | masquer avant `trailing_zeros` |
| 5 | Sélection d'id inversée dans `mesh_axis` | mauvaises textures/AO | « hi solide » → `solid_block_id(d)`, « lo solide » → `neighbor` si `d==0` sinon `solid_block_id(d-1)` |
| 6 | `transpose32` delta-swap | transposition fausse | transposition naïve + test |
| 7 | Origine de chunk = coords de chunk | tout rendu empilé à l'origine, flicker | `origin = coords * CHUNK_SIZE` |

---

## 6. Objectifs futurs

### 6.1 Mesurer / profiler
- Overlay in-game **Ctrl+O** : `TIME_SPENT_MESHING_NANOS / CHUNKS_MESHED` (moyenne par chunk).
- Micro-bench : ajouter un `#[test]` `--nocapture` qui boucle `make_greedy_chunk` sur des snapshots
  variés et imprime ns/chunk (médiane sur N itérations).
- `perf`/`flamegraph` : cible `Makefile` (`make flamegraph`, `make profile-main`).
- Isoler le coût : `ChunkSolidity::new` (build) vs `mesh_axis` (masquage+AO+fusion) vs émission.

### 6.2 Optimisation vers < 100 µs/chunk
Cible : **< 100 µs/chunk** (actuellement ~403 µs). Pistes, par ordre de priorité estimé :

1. **Profiler d'abord** pour lever le doute sur le poste dominant. À ~403 µs, il est possible que la
   fusion/émission (parcours + écriture `Vec<Vertex>`) domine désormais, pas la solidité.
2. **Construction de la solidité** :
   - `build_core` est déjà SIMD ; vérifier le coût des transpositions (`transpose32` naïve, 64×).
     Re-SIMD la transposition (butterfly bien compris cette fois) **en la testant** vs la version naïve.
   - Éviter de reconstruire `core_t`/`core_x` si non nécessaires, ou les construire directement depuis
     le snapshot.
   - Les voisins : `build_neighbor_layers` fait des boucles scalaires par bit ; possible SIMD.
3. **Masquage + AO** : `face_ao` lit des bits un par un (scalaire). Vectoriser les 3 lectures par coin
   (gather/shift sur `u64`). Calculer AO par octets plutôt que par coins.
4. **Fusion** : `match_row_avx2` avance de 4 lanes ; on peut traiter les 32 clés avec 2 registres
   AVX2 de 256 bits, ou passer à AVX-512 si dispo. Éviter de réécrire `keys` entièrement à chaque plan
   (seules les cellules de face changent).
5. **Émission** : réserver `vertices` (`reserve`) et écrire les 6 `Vertex` directement par tranches,
   ou batcher par plan. Éviter `extend_from_slice` de petites tranches si coûteux.
6. **Skip de plans vides** : si `lo_core == hi_core` pour toutes les lignes, sauter le plan sans
   calculer les clés (fréquent sur chunk vide / uniforme).
7. **Shift padding** : le `>> 1` sur chaque ligne de plan pourrait être évité en paddant autrement.

### 6.3 Dette technique / hors périmètre
- `chunk.rs` (working tree) contient un `PaddedChunkSliceMask` **cassé** (34 bits/u32). La version
  commitée `44334a4` a-t-elle rectifié ce fichier ? À vérifier ; la référence de test est la version
  HEAD `ef82588` (bool array).
- `FaceMask` et `PaddedChunk` deviennent inutiles en production (encore utilisés par la référence de test).
- `BlockInstance::is_solid()` = `id != 0` ⇒ l'eau (`"solid": false`) est traitée comme solide
  (faces + collision). Bug **connexe**, non corrigé (choix assumé pour l'égalité bit-à-bit).
- UV pivotées 90° entre X/Z et Y ; logique de flip AO commentée dans l'ancien `make_greedy_z`.
  Non corrigé (hors périmètre).
- `client/game.rs` utilise `static mut` (warnings) pour l'instrumentation.

---

## 7. Reproduire l'environnement

- Workspace Rust stable, crates : `client`, `game`, `engine`, `project_core` (+ `physics`, `network`).
- Cibles utiles (`Makefile`) : `fmt`, `check`, `test`, `profile-main`, `flamegraph`.
- Commandes de vérification :
  ```bash
  cargo check -p client --offline --all-targets
  cargo test  -p client --offline --release --lib chunk_new -- --test-threads=1   # 6 passed
  cargo test  -p project_core --offline                                            # 11 passed
  ```
- ⚠️ Toujours borner la mémoire des tests lourds (`--test-threads=1`, éventuellement `ulimit -v`).

---

## 8. Arborescence du code (repères `file:line`, commit `44334a4`)

- `chunk_new.rs:49` — constantes (`CHUNK`, `PAD`, `AO_TABLE`, indices voisins).
- `chunk_new.rs:70` — `pub enum Axis` (`faces`, `main_axis`, `row_axis`, `col_axis`, `reverse`).
- `chunk_new.rs:130` — `ao_neighbors` ; `:174` — `ao_lateral_offsets` ; `:209/:214` — `AO_OFFSETS_POS/NEG`.
- `chunk_new.rs:238` — `transpose32`.
- `chunk_new.rs:266` — `build_core` (+ `:300` scalar, `:316` SSE2, `:341` AVX2).
- `chunk_new.rs:393` — `build_neighbor_layers`.
- `chunk_new.rs:461` — `ChunkSolidity` + `:542-650` `build_plane_x/y/z` + `:654/:664` ids.
- `chunk_new.rs:693` — `match_row` (+ `:704` scalar, `:721` AVX2).
- `chunk_new.rs:756` — `face_ao`.
- `chunk_new.rs:777` — `mesh_axis` (masquage + AO + fusion + émission).
- `chunk_new.rs:950` — `pub fn make_greedy_chunk`.
- `chunk_new.rs:967+` — `mod tests` (`:982` `struct Reference` + 6 tests).
- `processor.rs:27` — appel `make_greedy_chunk` avec origine monde.
- `world.rs:38` — `BufferPool::with_max_buffers`.
- `buffer_pool.rs` — pool borné.
