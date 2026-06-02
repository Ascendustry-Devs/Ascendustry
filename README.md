# Ascendustry

Jeu voxel multijoueur en Rust avec rendu wgpu.

```
Ascendustry/
├── shared/    -- Bibliothèque partagée (réseau, monde, blocs, chiffrement)
├── client/    -- Client de jeu (rendu wgpu, audio, physique, joueur)
└── server/    -- Serveur multijoueur (monde, connexions, génération)
```

## Dépendances principales

- **Rust** édition 2021
- **wgpu** 28 — rendu GPU (Vulkan/Metal/DX12)
- **winit** 0.30 — fenêtrage et entrées
- **tokio** — réseau asynchrone
- **kira** 0.12 — audio
- **serde** + **bincode** — sérialisation binaire
- **aes-gcm** — chiffrement AES-256-GCM
- **noise** 0.9 — génération procédurale de terrain
- **cgmath** — mathématiques 3D
- **rayon** — maillage parallèle des chunks

## Utilisation

```bash
# Lancer le serveur
make server

# Lancer le client
make client

# Tout lancer d'un coup (serveur arrière-plan + client)
make run

# Construire
make build
```

Les binaires acceptent `--address` / `-a` pour l'adresse de connexion (défaut : `127.0.0.1:42677`).

## Contrôles

- `&` — mode fil de fer
- `é` — bordures de chunks

## Licence

Copyright StrachyDev & Theora59-dev 2025-2026. Tous droits réservés.
