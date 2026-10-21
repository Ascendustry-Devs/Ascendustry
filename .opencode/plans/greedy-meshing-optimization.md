# Optimisation du greedy meshing voxel — Ascendustry

## 0. Constat (état actuel du working tree)

- `client/render/meshing/chunk.rs` (modifié, non commité) introduit `PaddedChunkSliceMask` : `[u32; 1156]`
  avec **34 bits par section** → impossible à tenir dans un `u32`.
  - `set_i_to` : `1 << (34 - inner_i)` → panic `attempt to shift left with overflow` en debug, masquage silencieux (`& 31`) en release.
  - `get_i` : lit le bit `32 - inner_i` → décalage incohérent avec `set_i_to` (aller-retour mesuré : **3616/4096 échecs**).
  - Conséquence : le rendu actuel est **faux** (faces mal détectées), pas seulement lent.
- `HEAD` (`ef82588`) utilise `&[bool; PADDED_CHUNK_BLOCK_CBE_USIZE]` (39 304 bools) : correct, mais lent
  (~1,1 ms/chunk observés).
- Le coût dominant n'est pas la fusion : c'est
  1. la **préparation** (39 304 écritures avec `div_euclid`/`%` par 34 — div/mod par non-puissance de 2) ;
  2. l'**AO** : 12 lectures de solidité par face, via `get_solidity_xyz` (arithmétique d'index 3D) ;
  3. la **masquage** cellule par cellule.

## 1. Faits géométriques établis (à revérifier par le test d'équivalence)

- Pour un axe donné, une face à la plan `d` (entre `d` et `d+1`) a ses 3 échantillons AO **dans le plan du côté
  air** : plan `d+1` pour une face `+axe`, plan `d` pour une face `-axe`.
- Les échantillons AO d'une face sont donc toujours dans la région paddée `[0..=33]³` → le padding d'une couche
  suffit, **pas de débordement hors chunk** (les 8 coins de chunk restent à l'air, comme aujourd'hui).
- AO : `ao = 0` si `side1 && side2`, sinon `ao = 3 - side1 - side2 - corner` (équivalent à `AO_TABLE`).
- Par coin : `corner` = voisin diagonal, `side1`/`side2` = les 2 voisins orthogonaux (3×3 dans le plan de la face).
- Les décalages exacts par (axe, direction, coin) sont **mécaniquement dérivés** de `UVNORMAL_TABLE`/`CORNER_TABLE`
  existants → à re-exprimer en constantes `(row_delta, bit_delta)` (aucune nouvelle géométrie).

## 2. Architecture cible

### 2.1 Solidity : 3 familles de plans bit

```
const PLANES: usize = 33;   // plans 0..=32  (face planes d=0..=31 + plan AO d+1)
const ROWS:   usize = 34;   // lignes 0..=33 (coordonnée tangente paddée)
const COLS:   usize = 34;   // bits   0..=33 (coordonnée tangente paddée)

struct AxisPlanes { planes: [[u64; ROWS]; PLANES] }   // 8 976 o
struct ChunkSolidity { x: AxisPlanes, y: AxisPlanes, z: AxisPlanes }  // ~27 ko
```

- 1 mot `u64` par ligne ⇒ 34 bits ⇒ **plus de débordement possible**, accès par masque/shift (pas de div/mod).
- 33 plans par axe : le plan `p` contient la solidité du plan de bloc `p` dans les 2 axes tangents.
  - Axe X : plan `x`, lignes `y`, bits `z`.
  - Axe Y : plan `y`, lignes `z`, bits `x`.
  - Axe Z : plan `z`, lignes `x`, bits `y`.
- ~27 ko par chunk meshing : tient dans le stack des workers (8 Mo, `project_core/parallel.rs`).

### 2.2 Build (préparation)

1. 1 passe linéaire sur le `PaddedChunk` (x fastest) → mots de 34 bits le long de x
   (`cube[z][y]` = mot sur x) : 1 156 mots.
2. Axe Y : `M_Y[y][z] = cube[z][y]` ⇒ **transposition** de la matrice 34×34 de mots.
3. Axe Z : `M_Z[z]` = slice (x,y) du cube ⇒ transposition 34×34 par slice z.
4. Axe X : `M_X[x]` = slice (y,z) ⇒ idem.
   ⇒ 1 transposition de matrice de mots 34×34 (core 32×32 par butterfly 5 étages, bordures 32/33 traitées à part),
   par famille d'axes.
   Budget : ~40 k opérations simples + 3 transpositions ⇒ visé < 60 µs/chunk.
   (Alternative plus simple si le build devient le goulot : construire les 3 familles directement depuis
   `MeshSnapshot` (chunk + 6 voisins) au lieu du `PaddedChunk` 157 ko — à mesurer.)

### 2.3 Masquage + AO par axe (générique)

Pour chaque plan de face `d` ∈ `0..32` et chaque ligne `r` ∈ `0..32` :

```
A = planes[d].row(r)          // solidité du plan d
B = planes[d+1].row(r)        // solidité du plan d+1
face_pos = A & !B             // bloc solide en d      → face +axe (AO dans le plan d+1)
face_neg = !A & B             // bloc solide en d+1    → face -axe (AO dans le plan d)
```

AO par cellule de face (extraction de bits depuis 3 mots `u64`registers : lignes `r-1`, `r`, `r+1` de l'axe) :
`side1`, `side2`, `corner` par coin, puis `ao = 0 si s1&s2 sinon 3-s1-s2-c`.
Objectif : ~10 opérations par cellule de face au lieu de 12 accès mémoire indexés.

Rejets de plans vides : si `A == B` sur les 32 lignes utiles ⇒ aucun plan de face ⇒ on saute le plan entier
(les cas « chunk plein d'air » ou « bloc uniforme » sont fréquents).

### 2.4 Fusion greedy

- Conserver la **sémantique** actuelle : fusionner seulement si `texture + direction + AO` sont identiques
  (pas de relaxation ⇒ pas de changement visuel).
- Conserver l'**ordre de parcours** actuel (`u` lignes outer, `v` colonnes inner, saut `v += height`) ⇒
  l'ordre des triangles dans le buffer reste identique ⇒ égalité bit-à-bit possible.
- Représentation : par plan de face, `keys: [[u32; 32]; 32]` (clé = `tex<<16 | dir<<8 | ao`) + `visited` en
  bitmap `[u64; 32]` (1 ligne = 1 mot) au lieu du bit `visited` dans le `FaceMask`.
  Suppression possible de `FaceMask` (`client/render/utils/face_mask.rs`) qui devient inutile.

### 2.5 Émission

- Table par axe (positions des 4 coins, mapping UV, règle de `reverse`) pour fusionner
  `make_greedy_x/y/z` en **une seule fonction générique** `make_greedy_axis(axis, …)`.
  - X : lignes = Y, colonnes = Z ; `reverse = dir.is_negative()` ; UV u = extent Y, UV v = extent Z.
  - Y : lignes = Z, colonnes = X ; `reverse = dir.is_positive()` ; UV u = extent X, UV v = extent Z.
  - Z : lignes = X, colonnes = Y ; `reverse = dir.is_positive()` ; UV u = extent Y, UV v = extent X.
- `Vertex` fait 32 o ⇒ 6 sommets = 192 o par quad ; conserver `extend_from_slice` (déjà rapide) ou
  reservation + écriture directe si le profile montre un coût.

## 3. Étapes d'implémentation

| # | Étape | Contenu | Critère de fin |
|---|-------|---------|----------------|
| 0 | Harnais de test | `client/render/meshing/tests.rs` : générateur de chunks déterministes (vide, cube, slab, bruit, bord sans voisins, avec voisins) + **référence** = implémentation `HEAD` (`&[bool; 39304]`) conservée en `#[cfg(test)]` | comparaison `Vec<Vertex>` **bit-à-bit** |
| 1 | `ChunkSolidity` | nouveau module `client/render/meshing/solidity.rs` : structures, build (cube + transpositions), tests unitaires (round-trip, bornes) | test d'équivalence du masque vs bool |
| 2 | Masquage + AO générique | 1 fonction pour les 3 axes, plans bit, AO par cellule | équivalence bit-à-bit sur N chunks |
| 3 | Fusion + émission génériques | remplace `make_greedy_x/y/z` | équivalence bit-à-bit (ordre des triangles inclus) |
| 4 | Nettoyage | suppression de `PaddedChunkSliceMask` et `FaceMask`, adaptation de `processor.rs` | `cargo clippy` propre |
| 5 | Build optimisé | mesure du build ; variante directe depuis `MeshSnapshot` si le `PaddedChunk` (157 ko) domine | build < 60 µs/chunk |
| 6 | SIMD (optionnel) | `std::arch` + `is_x86_feature_detected!` : AVX2 pour le build (gather 34 bits), SSE2 pour le masquage/AO ; fallback scalaire | gain mesuré > 15 % |
| 7 | Mesure | micro-bench (`#[test]` + `--nocapture`) + overlay Ctrl+O in-game (`TIME_SPENT_PREPARING_NANOS`, `TIME_SPENT_MESHING_NANOS`) | rapport avant/après |

## 4. Validation

- `cargo test -p client` : équivalence bit-à-bit nouveau vs référence (≥ 8 scénarios déterministes, seed fixe),
  + test de round-trip du masque (4096 bits) + tests de bornes.
- `cargo clippy --all-targets` + `cargo fmt --check`.
- Mesure in-game : overlay Ctrl+O (chunks meshés, ns de préparation, ns de meshing).
- Pas de validation GPU possible dans cet environnement ⇒ l'égalité bit-à-bit avec la référence est le seul garantie
  objective ; le contrôle visuel final reste à faire par l'utilisateur.

## 5. Risques / points d'attention

- **Transposition 34×34** : les bordures (indices 32/33) sont la source d'erreur classique → traitement explicite
  et testé séparément (noyau 32×32 par butterfly 5 étages + 2 lignes/colonnes à part).
- **Ordre de la fusion greedy** : l'algorithme n'est pas canonique ; changer l'ordre de parcours change l'ordre des
  triangles (pas la géométrie, mais l'égalité bit-à-bit casse). ⇒ garder l'ordre actuel.
- **Bords de chunk** : l'AO dépend des 6 voisins ; les 8 coins de chunk restent à l'air (comportement actuel,
  pas de régression) — un correctif nécessite les chunks diagonaux (hors snapshot actuel).
- **Eau / `is_solid`** : `BlockInstance::is_solid()` = `id != 0`, donc l'eau (`"solid": false` dans le JSON) génère
  des faces et est solide pour la collision. Bug **connexe**, hors périmètre par défaut (cf. question Q2).
- **UV incohérents** : textures pivotées 90° entre X/Z et Y, logique de flip AO commentée dans `make_greedy_z`.
  Hors périmètre par défaut (cf. question Q2).

## 6. Questions à trancher avant implémentation

- **Q1 — Périmètre** : (a) correction minimale du masque cassé seulement, ou (b) refonte complète
  (solu bit + masquage/AO/fusion génériques) ? Recommandation : (b), par étapes, avec l'étape 0 d'abord.
- **Q2 — Tolérance visuelle** : la sortie doit-elle être **strictement identique** à celle d'aujourd'hui (recommandé :
  oui, hors correctifs de bugs assumés), ou peut-on corriger en même temps l'eau (`is_solid`), les UV pivotées et
  l'AO aux coins de chunk ?
- **Q3 — SIMD** : autoriser `std::arch` + dispatch AVX2/SSE2 (x86) avec fallback scalaire, ou rester 100 % portable ?
- **Q4 — Cible de performance** : quel seuil à valider (µs par chunk, ou chunks/s à N workers) ?
