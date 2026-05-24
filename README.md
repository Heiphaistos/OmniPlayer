# OmniPlayer — Lecteur Multimédia Universel

![Version](https://img.shields.io/badge/version-1.3.0-blue)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?logo=rust)
![Go](https://img.shields.io/badge/Go-1.22-00ADD8?logo=go)
![License](https://img.shields.io/badge/licence-MIT-green)
![Platform](https://img.shields.io/badge/platform-Windows-0078D4?logo=windows)

Lecteur multimédia haute performance construit sur un pipeline Rust/wgpu, avec décodage FFmpeg, rendu GPU natif via wgpu/DirectX 12, et des services annexes en Go (sous-titres en ligne, indexation de bibliothèque).

---

## Fonctionnalités

- **Décodage universel** — tous les formats via FFmpeg 7.x (H.264, H.265, AV1, VP9, MPEG-2, etc.)
- **Rendu GPU** — pipeline wgpu/DirectX 12 avec conversion YUV→RGB en shader WGSL
- **Espace colorimétrique auto** — détection et conversion BT.601 / BT.709 / BT.2020 à la volée
- **HDR** — détection PQ (SMPTE 2084) et HLG (ARIB STD-B67), badge HDR dans l'interface
- **Accélération matérielle** — DXVA2, D3D11VA (sélection automatique au démarrage), NVDEC, AMF, QuickSync
- **Visionneuse d'images** — mode dédié pour JPEG, PNG, WebP, TIFF, AVIF, HEIC, RAW, et 30+ formats photo
- **Lecture réseau** — ouverture d'URL directe (HTTP, RTMP, HLS `.m3u8`, DASH `.mpd`)
- **Sous-titres** — chargement externe (SRT, ASS, SSA, VTT) + chargement automatique du fichier adjacent + recherche via OpenSubtitles
- **Multi-pistes** — changement de piste audio et de piste sous-titre à la volée (touche A / S)
- **Chapitres** — navigation par chapitre (marqueurs visuels sur la seekbar), navigation précédent/suivant
- **Playlist** — glisser-déposer, navigation clavier, boucle Off/×1/Tout
- **Vitesse variable** — 0.25×, 0.5×, 0.75×, 1×, 1.25×, 1.5×, 2×, 4×
- **Plein écran** — auto-hide des contrôles après 3 secondes, escape pour sortir
- **Fichiers récents** — historique des 20 derniers fichiers
- **Overlay d'infos** (touche I) — résolution, codec, fréquence d'images, canal audio, HDR, temps de buffer
- **Métadonnées TMDB** — enrichissement automatique des films/séries via le service Go
- **Bibliothèque médias** — indexation de dossiers locaux via l'indexeur Go, recherche par titre
- **Format image Fit/Fill/Stretch** — basculement cyclique (touche W)
- **Volume jusqu'à 150%** — amplification logicielle

---

## Formats Supportés

### Vidéo (conteneurs)
`mp4` `mkv` `avi` `mov` `wmv` `flv` `webm` `ts` `m2ts` `mts` `mpg` `mpeg` `m4v` `3gp` `3g2` `ogv` `rm` `rmvb` `divx` `xvid` `vob` `ifo` `f4v` `asf` `mxf` `dv` `m2v`

### Vidéo (codecs bruts)
`h264` `h265` / `hevc` `264` `265` `avc` `vc1` `av1` `ivf` `nuv` `nsv` `roq` `drc`

### Audio
`mp3` `aac` `flac` `ogg` `opus` `wav` `wma` `m4a` `ape` `mka` `mpa` `ac3` `eac3` `dts` `dtshd` `mlp` `truehd` `mp2` `mp1` `wv` `tta` `aiff` `aif` `au` `snd` `caf` `spx` `mpc` `ra` `amr` `gsm` `voc`

### Streaming
`m3u8` (HLS) `mpd` (DASH) `m3u`

### Images
`jpg` `jpeg` `png` `gif` `bmp` `webp` `tiff` `ico` `pnm` `pbm` `pgm` `ppm` `tga` `hdr` `avif` `heic` `heif` `jxl` `qoi`

### Formats RAW photo
`raw` `cr2` `cr3` `nef` `arw` `dng` `orf` `rw2` `pef` `srw` `x3f` `raf` `nrw`

---

## Captures d'écran

> Les captures seront ajoutées lors de la première release publique.

---

## Installation (binaires précompilés)

1. Téléchargez la dernière archive `OmniPlayer_vX.Y.Z_Portable.zip` depuis la [page Releases](https://github.com/heiphaistos44-crypto/OmniPlayer/releases).
2. Extrayez dans le dossier de votre choix.
3. Lancez `launch.bat` (démarre les services Go + le lecteur) **ou** `OmniPlayer.exe` directement (sans les services sous-titres/indexeur).

> **Clés API optionnelles** — pour activer la recherche de sous-titres et les métadonnées TMDB, renseignez vos clés dans `launch.bat` ou via les variables d'environnement :
> ```
> set OPENSUBTITLES_API_KEY=votre_cle
> set TMDB_API_KEY=votre_cle
> ```

---

## Build depuis les sources

### Prérequis

| Outil | Version minimale | Rôle |
|-------|-----------------|------|
| Rust + Cargo | 1.75 (stable MSVC) | Compilation du lecteur principal |
| Go | 1.22 | Compilation des services annexes |
| FFmpeg | 7.x (shared libs) | Décodage audio/vidéo |
| Visual Studio Build Tools | 2022 | Toolchain MSVC pour Rust |

### Installation automatique (Windows)

Le script `setup.bat` vérifie et installe les dépendances manquantes :

```bat
setup.bat
```

Ce script :
- Vérifie et installe Rust (via rustup) avec les cibles x64 et x32
- Vérifie que Go est disponible
- Télécharge FFmpeg 7.x depuis BtbN et l'installe dans `C:\ffmpeg`
- Configure les variables d'environnement `FFMPEG_DIR` et `PATH`

### Compilation

```bat
REM Build complet (release x64 — recommandé)
build.bat

REM Options disponibles
build.bat debug           # Mode debug
build.bat x32             # Architecture 32 bits
build.bat skip-go         # Ignorer la compilation Go
build.bat release x32     # Release 32 bits
```

La commande génère dans `dist\` :
- `OmniPlayer.exe` — lecteur principal
- `subtitle-service.exe` — service sous-titres/TMDB (Go)
- `media-indexer.exe` — indexeur de bibliothèque (Go)
- Les DLLs FFmpeg nécessaires au runtime

### Compilation manuelle (développement)

```bat
REM Rust uniquement (mode développement rapide)
run.bat

REM Ou manuellement
set FFMPEG_DIR=C:\ffmpeg
cargo run -p omni-player

REM Services Go
go build -o dist\subtitle-service.exe .\cmd\subtitle-service\
go build -o dist\media-indexer.exe .\cmd\media-indexer\
```

---

## Architecture

OmniPlayer suit une architecture multi-processus : un lecteur Rust temps réel avec pipeline de décodage multithread, et deux services Go légers communiquant via HTTP local.

```
┌─────────────────────────────────────────────────────────┐
│                   OmniPlayer.exe (Rust)                  │
│                                                          │
│  ┌───────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ omni-core │  │omni-renderer │  │   omni-audio     │  │
│  │ (FFmpeg)  │  │  (wgpu/DX12) │  │   (CPAL/rubato)  │  │
│  └─────┬─────┘  └──────┬───────┘  └──────┬───────────┘  │
│        │               │                  │               │
│  ┌─────▼───────────────▼──────────────────▼───────────┐  │
│  │               omni-player (egui/eframe)              │  │
│  │        Interface utilisateur + orchestration         │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
          │ HTTP (127.0.0.1)          │ HTTP (127.0.0.1)
          ▼ :18080                    ▼ :18081
┌──────────────────────┐   ┌──────────────────────────┐
│  subtitle-service.go │   │   media-indexer.go        │
│  OpenSubtitles + TMDB│   │  Scan dossiers + TMDB     │
└──────────────────────┘   └──────────────────────────┘
```

### Crates Rust

#### `omni-core` — Moteur de décodage

Le cœur du lecteur. Gère le démultiplexage, le décodage audio/vidéo, la synchronisation A/V et le probing de fichiers.

- **`probe.rs`** — Extraction des métadonnées sans décodage complet (codec, résolution, FPS, HDR, chapitres, langues audio, pistes de sous-titres)
- **`pipeline/mod.rs`** — Orchestrateur `MediaPipeline` : lance le thread demuxer, expose des canaux crossbeam typés pour les frames vidéo/audio et les événements
- **`pipeline/demuxer.rs`** — Thread dédié de démultiplexage FFmpeg. Gère les commandes (Seek, Pause, Stop, SelectAudioTrack) et distribue les paquets aux décodeurs. Préfère D3D11VA à DXVA2 sur Windows 8+
- **`pipeline/clock.rs`** — Horloge maître A/V avec gestion de la vitesse de lecture
- **`decoder/video.rs`** — Décodeur vidéo avec SwsContext pour la conversion de format. Reconstruit le scaler automatiquement en cas de changement de résolution mid-stream
- **`decoder/audio.rs`** — Décodeur audio exposant des frames PCM float32
- **`decoder/subtitle.rs`** — Parseur de sous-titres SRT et ASS/SSA en mémoire
- **`hw_accel/mod.rs`** — Sélection de l'accélération matérielle (DXVA2, D3D11VA, CUDA) avec threading Frame-level (4 workers)

#### `omni-renderer` — Rendu GPU

Rendu YUV→RGB en temps réel via wgpu (backend DirectX 12 / Vulkan).

- **`video_renderer.rs`** — Pipeline wgpu complet : upload des textures YUV planes, bind groups, render pipeline avec shader WGSL. Supporte BT.601, BT.709 et BT.2020 via uniforms GPU mis à jour dynamiquement
- **`frame_upload.rs`** — Upload efficace des plans Y/U/V vers des textures GPU distinctes
- **`hdr.rs`** — Utilitaires de tone mapping HDR
- **`shaders/yuv_to_rgb.wgsl`** — Shader WGSL : vertex fullscreen quad + fragment YUV→RGB avec matrice colorimétrique configurable

#### `omni-audio` — Moteur audio

Sortie audio temps réel via CPAL avec rééchantillonnage automatique.

- **`output.rs`** — `AudioEngine` : ring buffer de 8 secondes, thread `audio-fill` dédié au rééchantillonnage, callback CPAL non-bloquant. Supporte F32, I16, U16, I32, F64, U32, I8, U8. Downmix automatique vers stéréo (mono, 5.1, 7.1)
- **`resampler.rs`** — Rééchantillonnage de haute qualité via `rubato`

#### `omni-player` — Application principale

Interface utilisateur egui + orchestration de tous les composants.

- **`app.rs`** — `OmniApp` : boucle principale egui, gestion du clavier, drag-and-drop, synchronisation A/V, gestion de playlist, mode plein écran, OSD
- **`player.rs`** — `Player` : API haut niveau (open, seek, play_pause, volume, sous-titres, pistes audio/sous-titres, chapitres)
- **`config.rs`** — Configuration persistante JSON (`%APPDATA%\OmniPlayer\config.json`) : fenêtre, volume, mode boucle, mode aspect, vitesse, fichiers récents
- **`services.rs`** — Client HTTP pour les services Go (recherche/téléchargement de sous-titres, bibliothèque médias)
- **`ui/controls.rs`** — Barre de contrôles avec gradient de fond, seekbar custom avec marqueurs de chapitres et tooltip temporel, badges HDR/résolution
- **`ui/player_view.rs`** — Zone vidéo centrale (rendu wgpu callback ou image viewer)
- **`ui/file_browser.rs`** — Explorateur de fichiers intégré
- **`ui/info_overlay.rs`** — Overlay diagnostics (codec, résolution, FPS, buffer, espace colorimétrique)
- **`ui/playlist.rs`** — Panneau playlist latéral redimensionnable
- **`ui/settings.rs`** — Fenêtre de paramètres
- **`ui/url_dialog.rs`** — Dialogue ouverture URL réseau

### Module Go

Deux services HTTP légers exposés uniquement sur `127.0.0.1` (loopback uniquement).

#### `cmd/subtitle-service` + `pkg/ipc` + `pkg/subtitles`
Pont HTTP vers OpenSubtitles v3 et TMDB. Endpoints :
- `GET /health` — health check
- `GET /subtitles/search?filename=...&lang=fr` — recherche de sous-titres
- `POST /subtitles/download` — téléchargement (limité à 10 MB par fichier, body limité à 4 KB)
- `GET /metadata/movie?title=...` et `GET /metadata/tv?title=...` — métadonnées TMDB

#### `cmd/media-indexer` + `pkg/metadata`
Indexeur de bibliothèque médias locale. Endpoints :
- `GET /library` — retourne la liste indexée
- `POST /index` — déclenche une ré-indexation (body limité à 64 KB)
- `GET /search?q=...` — recherche textuelle (max 50 résultats)

### Flux de données

```
Fichier/URL
    │
    ▼
probe_file()  ─────────────────────────────► MetadataReady (MediaInfo)
    │                                               │
    ▼                                               ▼
MediaPipeline::launch()                        Player.media_info
    │
    ├─► Thread "omni-demuxer"
    │       │
    │       ├── FFmpeg packet read loop
    │       │       │
    │       │       ├─► VideoDecoder → SwsContext → DecodedVideoFrame
    │       │       │       └── channel bounded(16) ──► pump_video()
    │       │       │                                        │
    │       │       │                               MasterClock sync
    │       │       │                                        │
    │       │       │                               VideoRenderer.upload_frame()
    │       │       │                                        │
    │       │       │                               wgpu render pass (YUV→RGB WGSL)
    │       │       │
    │       │       └─► AudioDecoder → DecodedAudioFrame
    │       │               └── channel bounded(512) ──► pump_audio()
    │       │                                                │
    │       │                                        AudioEngine.push_frame()
    │       │                                                │
    │       │                                        Thread "audio-fill"
    │       │                                        rubato resampler → ring buffer
    │       │                                                │
    │       │                                        CPAL callback → haut-parleurs
    │       │
    │       └── PipelineEvent → bounded(64) ──► poll_events()
    │                                               │
    │                                        PlayerState machine
    │                                        (Loading→Playing→EOF)
    │
    └─► Commandes (Seek, Pause, Resume, Stop, SelectAudioTrack)
            cmd_tx.try_send() depuis le thread UI
```

---

## Raccourcis Clavier

| Touche | Action |
|--------|--------|
| `Espace` | Lecture / Pause |
| `←` | Reculer de 10 s |
| `→` | Avancer de 10 s |
| `Shift + ←` | Reculer de 60 s |
| `Shift + →` | Avancer de 60 s |
| `Alt + ←` | Reculer de 1 s |
| `Alt + →` | Avancer de 1 s |
| `↑` | Volume +10% |
| `↓` | Volume −10% |
| `M` | Muet / Activer le son |
| `A` | Piste audio suivante |
| `S` | Piste sous-titres suivante |
| `N` | Média suivant (playlist) |
| `P` | Média précédent (playlist) |
| `[` | Vitesse de lecture −1 cran |
| `]` | Vitesse de lecture +1 cran |
| `W` | Basculer Fit / Fill / Stretch |
| `L` | Basculer mode boucle (Off / ×1 / Tout) |
| `F` | Basculer plein écran |
| `Échap` | Quitter le plein écran |
| `I` | Afficher/masquer l'overlay d'infos |
| `Ctrl + O` | Ouvrir un fichier (explorateur) |
| `Ctrl + L` | Ouvrir une URL réseau |
| `Ctrl + P` | Afficher/masquer la playlist |
| `Ctrl + Q` | Quitter l'application |

---

## Stack Technique

| Composant | Technologie | Rôle |
|-----------|-------------|------|
| Langage principal | Rust 2021 | Lecteur, décodage, rendu |
| Services annexes | Go 1.22 | Sous-titres, indexeur, TMDB |
| GUI | egui 0.31 + eframe 0.31 | Interface utilisateur immédiate |
| GPU | wgpu 24 (DX12 + Vulkan) | Rendu vidéo GPU |
| Shader | WGSL | Conversion YUV→RGB |
| Décodage | FFmpeg 7.x (ffmpeg-next 8) | Audio + vidéo + sous-titres |
| Audio output | CPAL 0.15 | Abstraction hardware audio |
| Rééchantillonnage | rubato 0.15 | Conversion sample rate |
| Canaux | crossbeam-channel 0.5 | Communication inter-threads |
| Ring buffer | ringbuf 0.3 | Buffer audio lock-free |
| Mutexes | parking_lot 0.12 | Mutexes haute performance |
| Fichier dialog | rfd 0.14 | Dialogue natif Windows |
| HTTP client (Rust) | reqwest 0.12 | Appels aux services Go |
| HTTP server (Go) | stdlib net/http | Services loopback |
| Configuration | serde_json | Persistance JSON |

---

## Problèmes Connus

### Fonctionnalités non encore implémentées

- **Commutation de piste sous-titre mid-stream** — La commande `SelectSubtitleTrack` est envoyée mais non traitée côté demuxer. Les pistes de sous-titres intégrées au conteneur ne sont pas routées. Seuls les fichiers SRT/ASS/SSA chargés externement fonctionnent.
- **D3D11VA zero-copy** — L'accélération D3D11VA applique du threading frame-level (4 workers) mais ne réalise pas de décodage GPU zero-copy (pas de `AVHWDeviceContext`). Le bénéfice reste partiel (~15% vs DXVA2 sur H.265).
- **Enrichissement TMDB dans l'indexeur** — Le paramètre `--tmdb-key` est accepté mais non encore utilisé pour enrichir les entrées de la bibliothèque.

### Limitations connues

- **Windows uniquement** — Le projet utilise des APIs spécifiques Windows (D3D11VA, DXVA2, DirectX 12 via wgpu). Un portage Linux/macOS nécessiterait des adaptations sur l'accélération matérielle.
- **FFmpeg requis au runtime** — Les DLLs FFmpeg doivent être présentes dans `dist\` ou dans le `PATH`. Le build portable les inclut automatiquement.
- **Sous-titres bitmap (PGS, VOBSUB)** — Non supportés (rendu image complexe non implémenté). Seuls les sous-titres texte (SRT, ASS, SSA, VTT) sont pris en charge.

---

## Licence

MIT — voir [LICENSE](LICENSE)

---

*OmniPlayer v1.3.0 — Rust + Go + FFmpeg + wgpu*
