# OmniPlayer — Notes de Version

---

## v1.4.0 (2026-07-19) — Lecture fiable de bout en bout

### Corrections

- **[CRITIQUE] Sous-titres intégrés réellement affichés** — `poll_events` recevait `SubtitleLine` puis `update_subtitle()` écrasait aussitôt `current_subtitle` avec `None` dès qu'aucun sous-titre externe n'était chargé. Les événements intégrés sont maintenant mis en file `(texte, pts_start, pts_end)` et affichés à leur PTS exact (les paquets sont décodés en avance sur la lecture).
- **[CRITIQUE] Purge audio au seek et au changement de fichier** — le ring buffer (8 s) continuait de jouer l'ancien flux après un seek ou une ouverture, causant une désynchronisation A/V de plusieurs secondes. Nouveau `AudioEngine::flush()` (générations de frames + vidage du ring) déclenché via `Player::audio_flush_needed`.
- **[CRITIQUE] Régulation du débit de décodage** — le demuxer décodait tout le fichier à vitesse maximale : frames vidéo droppées (`try_send` sur queue pleine) et overflow du ring audio. Le demuxer attend désormais quand les queues aval sont pleines ; `pump_audio` vise ~4 s de ring ; `pump_video` draine aussi en `Loading` (pas de deadlock).
- **[HAUTE] Répétition ×1 réparée** — après fin de fichier, le thread demuxer est terminé : `seek(0)` partait dans le vide. `Player::replay()` relance un pipeline complet en préservant le sous-titre externe.
- **[HAUTE] Fin de fichier ne gèle plus le lecteur** — l'état reste `EndOfFile` ; Espace ou un seek relancent la lecture (`replay`), au lieu d'envoyer des commandes à un pipeline mort.
- **[MOYENNE] Erreurs demuxer remontées à l'UI** — une erreur en cours de lecture (seek impossible, flux corrompu) émettait seulement un log ; l'UI restait en `Playing` figé. L'événement `Error` est maintenant envoyé.
- Volume et vitesse de lecture restaurés au démarrage (ils étaient sauvegardés mais jamais relus).
- Drop de sous-titres insensible à la casse (`.SRT` accepté).
- `MasterClock::pause()` tient compte de la vitesse de lecture dans le snapshot de position.
- `build.bat` : détection des DLLs FFmpeg par joker (`avcodec-*.dll`) au lieu de la version 61 codée en dur.

### Nouveautés

- **Ouverture par ligne de commande** — `OmniPlayer.exe <fichier|URL>` : association de fichiers Windows et « Ouvrir avec » fonctionnels.

---

## v1.3.1 (2026-05-24) — Correctifs

### Corrections

- **[CRITIQUE] Sous-titres intégrés MKV/MP4 fonctionnels** — `PipelineCommand::SelectSubtitleTrack` était silencieusement ignoré (`_ => {}`). Désormais, le demuxer crée un décodeur de sous-titres FFmpeg dédié (`ffmpeg::codec::decoder::Subtitle`), filtre les paquets du stream sélectionné, extrait le texte des rects `Text` et `Ass` via les helpers `collect_subtitle_text` / `strip_ass_overrides`, et émet `PipelineEvent::SubtitleLine(texte, pts_début, pts_fin)`. Les sous-titres PGS/bitmap restent non supportés (ignorés proprement). Nouveau variant `SubtitleLine` dans l'enum `PipelineEvent`.
- **Icône application** — `load_icon()` génère maintenant un cercle 32×32 avec dégradé horizontal bleu (#0080FF) → violet (#8000FF) et fond transparent, visible dans la barre des tâches Windows. Remplace les 32×32 pixels noirs précédents.
- **Services Go silencieux** — si le service sous-titres ne répond pas au health check au démarrage, un `log::warn!` est émis et un message OSD s'affiche 2,5 secondes : *"Services sous-titres non disponibles — lancez launch.bat pour les activer"*. L'absence de services ne passe plus inaperçue.

---

## v1.3.0 (2026-05-24)

### Nouvelles fonctionnalités

- **Vitesse variable** — 8 niveaux de vitesse de lecture (0.25×, 0.5×, 0.75×, 1×, 1.25×, 1.5×, 2×, 4×) accessibles via les touches `[` / `]` ou le menu déroulant dans la barre de contrôles.
- **Mode boucle** — Trois modes configurables (Off, ×1, Tout) accessibles via la touche `L` et persistés dans la configuration.
- **Format image** — Bascule cyclique Fit / Fill / Stretch via la touche `W`, conservé entre les sessions.
- **Seek précis avec modificateurs** — `Alt + ←/→` pour ±1 s, `Shift + ←/→` pour ±60 s (en plus du ±10 s existant).
- **Auto-hide des contrôles** — En plein écran, les contrôles et la barre de menu se masquent automatiquement après 3 secondes d'inactivité de la souris.
- **OSD (On-Screen Display)** — Affichage temporaire (2,5 s) des actions clavier : changement de volume, seek, vitesse, mode boucle, format image, muet.
- **Navigation chapitres** — Boutons dédiés dans la barre de contrôles (⏮/⏭) et accès depuis le menu Lecture. Marqueurs visuels jaunes sur la seekbar avec tooltip de nom de chapitre.
- **Titre de fenêtre dynamique** — Le titre de la fenêtre reflète le fichier en cours de lecture.
- **Badge résolution** — Affichage SD / 480p / 720p / 1080p / 1440p / 4K UHD / 8K dans la barre de menu et la barre de contrôles.
- **Visionneuse d'images** — Mode dédié pour les images statiques avec zoom/pan interactif. Détecte automatiquement les extensions image et n'instancie pas le pipeline FFmpeg inutilement.
- **Drag-and-drop de sous-titres** — Glisser un fichier SRT/ASS/SSA/VTT directement sur la fenêtre pour le charger sur la vidéo en cours.
- **Chargement automatique de sous-titres** — Lors de l'ouverture d'un fichier vidéo, OmniPlayer cherche automatiquement un fichier SRT/ASS/SSA du même nom dans le même dossier.
- **Downmix surround** — Downmix 5.1 et 7.1 vers stéréo avec pondération correcte des canaux center et surround (ITU-R BS.775).
- **Accélération matérielle intelligente** — Sélection automatique D3D11VA (Windows 8+) ou DXVA2 au démarrage via `is_d3d11va_available()`.
- **Espace colorimétrique automatique** — Détection BT.601/BT.709/BT.2020 depuis les métadonnées FFmpeg avec heuristique de résolution en cas d'absence de métadonnées.
- **Métadonnées audio réelles** — Le panneau info (touche I) affiche désormais les vrais canaux, fréquence d'échantillonnage et débit des pistes audio (instantiation du décodeur audio lors du probe).

### Améliorations de l'interface

- Thème sombre avec accent bleu (#4A9EFF) cohérent sur tous les composants.
- Barre de contrôles avec gradient de fond quadratique (transparent → semi-opaque).
- Seekbar custom : thumb adaptatif (hover/drag), halo accent, marqueurs de chapitres, tooltip temporel avec nom de chapitre.
- Menu bar masqué en plein écran lors de l'auto-hide.
- Panneau playlist redimensionnable (largeur par défaut : 270 px).
- Highlight de drag-over : bordure accent (#4A9EFF) lors du survol de fichiers.
- Indicateur d'état buffering (pourcentage) et erreur (tronqué à 42 caractères) dans la barre de contrôles.
- Volume slider de 0 à 150% (amplification logicielle).

### Architecture

- Séparation claire en 4 crates Rust : `omni-core`, `omni-renderer`, `omni-audio`, `omni-player`.
- Pipeline multithread avec canaux crossbeam typés (vidéo: bounded(16), audio: bounded(512), événements: bounded(64), commandes: bounded(16)).
- Ring buffer audio de 8 secondes (HeapRb) avec thread `audio-fill` dédié — aucun traitement audio dans le callback CPAL.
- Shader WGSL unifié pour la conversion YUV→RGB avec uniforms GPU mis à jour dynamiquement selon l'espace colorimétrique.

---

## v1.2.0 (2026-05-20) — Release Audit

> Audit complet de sécurité et de robustesse. 11 problèmes résolus, 0 régression.

### Corrections critiques (CRASH)

- **CRASH-1** — Panic sur vidéo malformée (`crates/omni-core/src/decoder/video.rs`) : remplacement du `.expect("création SwsContext")` par une propagation `?` correcte. L'erreur est désormais remontée comme `PlayerState::Error(...)` sans crash.
- **CRASH-2** — Scaler obsolète lors d'un changement de résolution mid-stream (`crates/omni-core/src/decoder/video.rs`) : ajout des champs `scaler_src_w`, `scaler_src_h`, `scaler_src_fmt`. Le `SwsContext` est reconstruit automatiquement à chaque changement de géométrie ou de format pixel.

### Corrections haute sévérité (HIGH)

- **HIGH-1** — Téléchargement de sous-titres sans limite de taille (`pkg/subtitles/client.go`) : ajout de `io.LimitReader(fileResp.Body, 10*1024*1024)` — cap à 10 MB.
- **HIGH-2** — Corps HTTP non limité sur les services Go (`pkg/ipc/bridge.go`, `cmd/media-indexer/main.go`) : ajout de `http.MaxBytesReader` — 4 KB pour l'endpoint download, 64 KB pour l'endpoint index.
- **HIGH-3** — Construction du corps JSON via `fmt.Sprintf` (surface d'injection) (`pkg/subtitles/client.go`) : remplacement par `json.Marshal(map[string]int{"file_id": fileID})`.

### Corrections moyennes (MEDIUM)

- **MEDIUM-1** — Allocation heap dans le callback CPAL temps réel (`crates/omni-audio/src/output.rs`) : `scratch: Vec<f32>` pré-alloué et déplacé dans la fermeture ; plus aucune allocation après le premier callback. Élimine les glitches audio périodiques sur les périphériques I16/U16.
- **MEDIUM-2** — Métadonnées audio toujours à zéro dans le panneau info (`crates/omni-core/src/probe.rs`) : instanciation du décodeur audio pendant le probe pour extraire canaux, fréquence et débit réels.
- **MEDIUM-3** — D3D11VA et CUDA sans accélération effective (`crates/omni-core/src/hw_accel/mod.rs`) : threading Frame-level (4 workers) désormais appliqué pour tous les kinds HW (Dxva2, D3D11Va, Cuda).

### Corrections mineures (LOW)

- **LOW-1** — Préférence DXVA2 codée en dur (`crates/omni-core/src/pipeline/demuxer.rs`) : `is_d3d11va_available()` sondé au démarrage — D3D11VA sélectionné si disponible (~15% de throughput en plus sur H.265/HEVC).
- **LOW-2** — Dépendances Go inutilisées (`go.mod`) : suppression de `gorilla/mux`, `zerolog`, `cobra`, `diskv` via `go mod tidy`. `go.mod` contient désormais uniquement `go 1.22`.
- **LOW-3** — Qualité de code Go : `".mp4":".mp4"==".mp4"` → `".mp4": true`, `interface{}` → `any`, user-agent mis à jour vers `OmniPlayer v1.2`.

---

## v1.1.0 (2026-05-15) — Version initiale publique

> Première version fonctionnelle complète avec pipeline Rust + services Go.

### Fonctionnalités initiales

- Pipeline de décodage FFmpeg multithread (demuxer dédié, décodeurs vidéo et audio séparés).
- Rendu wgpu avec shader YUV→RGB WGSL et conversion BT.709 par défaut.
- Moteur audio CPAL avec ring buffer et rééchantillonnage rubato.
- Interface egui avec barre de contrôles, seekbar, playlist et explorateur de fichiers.
- Mode plein écran (touche F).
- Service sous-titres Go (OpenSubtitles v3 + TMDB) via HTTP loopback.
- Indexeur de bibliothèque médias Go avec scan récursif et recherche textuelle.
- Configuration persistante JSON.
- Scripts `setup.bat` et `build.bat` pour l'installation et la compilation Windows.
- Support de 100+ formats vidéo/audio/image via FFmpeg.
- Détection HDR (PQ / HLG) et badge HDR dans l'interface.
- Navigation par chapitres.
- Chargement de sous-titres externes (SRT, ASS, SSA, VTT).
- Drag-and-drop de fichiers médias.
- Historique des 20 derniers fichiers.
- Overlay d'informations techniques (touche I).

---

## v1.0.0 (2026-05-01) — Prototype interne

> Version initiale de preuve de concept. Non publiée publiquement.

- Pipeline FFmpeg basique (vidéo uniquement, format YUV420P uniquement).
- Rendu egui avec texture RGBA (conversion CPU).
- Audio via CPAL sans rééchantillonnage.
- Interface minimale (lecture, pause, seek).
- Pas de services Go, pas de sous-titres, pas de playlist.
