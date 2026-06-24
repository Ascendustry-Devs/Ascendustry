# Ascendustry

Jeu voxel multijoueur en Rust avec rendu wgpu.

```
Ascendustry/
├── project_core/  -- Types fondamentaux et structures partagées
├── game/          -- Monde, blocs, génération procédurale de terrain (noise)
├── engine/        -- Rendu GPU (wgpu), audio (kira), fenêtrage (winit)
├── network/       -- Réseau asynchrone (tokio), chiffrement AES-256-GCM
├── physics/       -- Physique et collisions (client & serveur)
├── client/        -- Client de jeu (rendu, audio, joueur)
├── server/        -- Serveur multijoueur (monde, connexions, logique)
└── launcher/      -- Lanceur graphique (eframe/egui)
```

## Dépendances principales

- **Rust** édition 2021
- **wgpu** 28 — rendu GPU (Vulkan/Metal/DX12)
- **winit** 0.30 — fenêtrage et entrées
- **tokio** — réseau asynchrone
- **kira** 0.12 — audio
- **serde** + **bincode** — sérialisation binaire
- **aes-gcm** + **sha2** — chiffrement AES-256-GCM
- **noise** 0.9 — génération procédurale de terrain
- **cgmath** — mathématiques 3D
- **rayon** — maillage parallèle des chunks
- **eframe** 0.27 — lanceur graphique
- **clap** — arguments en ligne de commande

## Fonctionnalités actuelles

- Génération procédurale du monde
    - Terrain
    - Grottes
- Contrôleur joueur séparé en deux modes
    - Normal
    - God-mode
- Interface basique (FPS display)
- Multijoueur
- Sauvegarde

## Technicité

- Génération du monde avec Simplex noise
- Greedy mesher pour simplifier les meshs de chunks
- Utilisation de worker pools pour les opérations parallélisables (génération, meshing)

## Utilisation

```bash
# ¹[client/server/launcher] 
# ²[profile/release]
# ³[build]

# Mode d'emploi
# Pour chaque commande, vous devez la préfixer par ¹ pour désigner la cible de la commande
# Vous pouvez rajouter ² en séparant ¹ et ² par un -, pour ajouter une option dans la commande
# Enfin, vous pouvez soit exécuter comme tel (ça va lancer), soit vous pouvez ajouter ³ en séparant ² et ³ par un -, pour seulement construire la cible

# Exemples
make launcher-release # Lance le launcher en mode release
make client-profile-build # Construit le client en mode profiler

# Commandes récurrentes

# Debug
make launcher

# Release
make launcher-release

# Client profile mode
make client-profile

# Documentation
make doc
```

Les binaires acceptent `--address` / `-a` pour l'adresse de connexion (défaut : `127.0.0.1:42677`).

## Profiling

```bash
# Besoins nécessaires
# - perf
# - cloner https://github.com/brendangregg/FlameGraph dans un dossier "Flamegraph" et le déplacer dans le projet courant.

# Vous devez d'abord lancer une cible client, de préférence avec l'option "profile" (c.f. ²).
# Ensuite, exécuter...

make profile-main # Profile uniquement le thread principal

# ou

make profile-all # Profile tous les threads (peut causer du bruit indésirable)

# cela va créer un fichier flamegraph.svg que vous pourrez ouvrir dans une visionneuse d'image (statique) ou votre navigateur (dynamique).

```

## Contrôles

### Commun aux 2 modes de jeu

- `ZQSD` — se déplacer
- `Left Click` — casser un bloc
- `G` — changer de mode de jeu

### Normal

- `Space` — sauter

### God-mode

- `Left Shift` — descendre verticalement
- `Space` — monter verticalement

### Autres

- `"` — sauvegarde (le chargement est effectué dès le lancement du jeu)

### Debug

- `&` — mode fil de fer (wireframe)
- `é` — bordures du chunk actuel
- `C` — informations sur le chunk actuel
- `Left CTRL + D` — CPU chunks meshs memory dump
- `V` — CPU & GPU chunks meshs memory overview

## Note

Attention, ce README peut être obsolète car nous le mettons à jour rarement.

## Licence

c.f. LICENCE.txt
