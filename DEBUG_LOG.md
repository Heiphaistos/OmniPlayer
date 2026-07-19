# OmniPlayer — Journal de débogage

Mis à jour en continu pendant la campagne de test sur PC réel (audio matériel).
Format par entrée : `[STATUT] Zone — description`. STATUT ∈ {FIXED, OPEN, TESTED-OK, TODO}.

---

## Bugs corrigés cette session (v1.4.0 → v1.4.3)

- [FIXED] Ctrl+L (ouvrir URL) déclenchait AUSSI le raccourci brut `L` (cycle mode répétition) — `k_l` ne vérifiait pas `!ctrl`. Même défaut sur Ctrl+P (ouvrait le panneau playlist ET appelait `playlist_prev()`). Trouvé par capture d'écran : l'OSD "Répétition : ×1" apparaissait derrière le dialogue URL fraîchement ouvert.
- [FIXED] Échap ne fermait aucun dialogue modal (URL, file browser, paramètres) — `k_esc` ne gérait que la sortie plein écran. Pire : le dialogue URL force le focus clavier du champ texte à CHAQUE frame (`resp.request_focus()` sans condition), donc `wants_keyboard_input()` reste vrai en continu → même en ajoutant la gestion d'Échap dans `handle_keyboard`, elle n'aurait jamais été atteinte pour ce dialogue précis. Fix : Échap lu directement dans `url_dialog.rs` (en amont du filtre global) ; ajout du cas fermeture file browser/paramètres dans `handle_keyboard`.

- [FIXED] Horloge pilotée par le curseur de décodage (~14s d'avance) au lieu de l'audio réellement joué → vidéo en avance sur l'audio.
- [FIXED] Ring audio (8s) pas purgé au seek/changement de fichier → désync multi-secondes après action utilisateur.
- [FIXED] `av_seek_frame` : ancien seek `format_ctx.seek(ts, ts..)` renvoyait EPERM sur MP4 → seek cassé.
- [FIXED] Sous-titres intégrés jamais affichés (`update_subtitle` écrasait `current_subtitle=None` en boucle).
- [FIXED] Parsing ASS/subrip : mauvais nombre de champs avant le texte (8 vs 9) → texte vide.
- [FIXED] Fin de fichier / répétition ×1 : pipeline mort après EOF, seek(0) ne faisait rien → lecteur figé.
- [FIXED] Erreurs demuxer jamais remontées à l'UI (log seul).
- [FIXED] Volume/vitesse sauvegardés mais jamais relus au démarrage.
- [FIXED] CRT dynamique (vcruntime140 manquant) → crash au lancement sur PC sans VC++ Redist. Passé en `+crt-static`.
- [FIXED] Son accéléré permanent : flux CPAL ouvert avec canaux natifs du device (ex. 6 en 5.1/7.1) alors que le pipeline downmixe toujours en stéréo → ring vidé N/2× trop vite. Flux forcé en stéréo.
- [FIXED] Fallback silencieux vers échantillons bruts si le resampler échoue à se créer → lecture à mauvaise fréquence indéfiniment.
- [FIXED] Deadlock démultiplexeur si pas de périphérique audio (`pump_audio` ne vidait pas la file du pipeline) → vidéo figée + seek mort après ~30s.
- [FIXED] Dérive horloge sans audio : `PositionChanged` recalait l'horloge sur chaque frame décodée au lieu de la laisser tourner en roue libre → retard croissant.

## Bugs ouverts / suspects (à vérifier sur ce PC)

- [OPEN] Aucun test réel de la sortie AUDIO multi-canaux (5.1/7.1) — fix v1.4.1 pas vérifié sur un vrai device surround (aucun disponible ici). Vérifier au moins que le stream s'ouvre correctement en stéréo forcé sans erreur sur ce PC (device standard).
- [OPEN] Piste audio multiple (`next_audio_track`) jamais testée avec un vrai fichier multi-pistes.
- [OPEN] Chapitres (navigation, marqueurs seekbar) jamais testés avec un vrai fichier ayant des chapitres.
- [OPEN] HDR (badge, tonemapping) jamais testé avec un vrai fichier HDR.
- [OPEN] Lecture réseau (HTTP/RTMP/HLS) mentionnée au README mais jamais testée.
- [OPEN] Recherche de sous-titres OpenSubtitles / métadonnées TMDB — nécessite clés API, jamais testé.
- [OPEN] Bibliothèque média (indexeur Go, port 18081) — jamais testée.
- [OPEN] Sous-titres bitmap (PGS/VOBSUB) — non supportés (connu, pas un bug).
- [OPEN] D3D11VA zero-copy GPU pipeline — non implémenté (connu, perf uniquement).

## Fonctionnalités testées sur PC hôte (audio réel) — 2026-07-19

- [TESTED-OK] Piste audio multi-track (touche A) — `test_multiaudio.mp4` (fr/eng), badge "♪ eng" correct après switch, process stable.
- [TESTED-OK] Chapitres — titre "Milieu" affiché, marqueurs jaunes sur seekbar aux bons offsets, boutons ⏮/⏭ apparaissent.
- [TESTED-OK] Sous-titre externe adjacent auto (.srt à côté du .mp4) — "Sous-titre externe DEUX" affiché au bon timing.
- [TESTED-OK] Vitesse [ / ] — 1×→1.25×→1.5×→1.25×→1×, badge vitesse correct à chaque étape, aucune anomalie après retour à 1×.
- [TESTED-OK] Volume ↓↓ + Mute (M) — OSD "Muet" + icône haut-parleur barrée corrects.
- [TESTED-OK] Format image (W) — Fit→Fill→Stretch→Fit, rendu visuellement correct (vidéo réellement étirée en mode Stretch, sans letterbox).
- [TESTED-OK] Plein écran (F) puis Échap — bascule réelle (barre de titre disparaît, résolution pleine 1920×1080, reconnu par l'overlay NVIDIA), retour fenêtré OK.
- [TESTED-OK] Overlay infos (I) — panneau complet et exact (conteneur, codec, résolution, débit, piste audio, espace couleur, vitesse, format, buffer, position clock).
- [TESTED-OK] Mode boucle (L) — Off→×1→All, icône + OSD corrects.
- [TESTED-OK] Visionneuse image PNG — centrée, badge résolution/qualité correct, échelle 100%.
- [TESTED-OK] EOF + replay (Espace) — confirmé dans campagne VM précédente, revalidé implicitement ici (process stable après tous les changements d'état).

## Testé — round 2 (dialogues, playlist, seekbar souris) — 2026-07-19

- [TESTED-OK] Ouverture URL (Ctrl+L) — dialogue s'ouvre, plus d'effet de bord loop, Échap ferme proprement.
- [TESTED-OK] File browser (Ctrl+O) — dialogue s'ouvre, Échap ferme proprement.
- [TESTED-OK] Paramètres (menu Outils > Paramètres, clic souris direct) — formulaire complet et cohérent (accel matérielle, tone mapping HDR, volume défaut, langue sous-titres, ports services Go, bibliothèque médias), Échap ferme proprement.
- [TESTED-OK] Playlist (Ctrl+P) — panneau s'ouvre avec l'entrée courante listée, plus d'effet de bord playlist_prev.
- [TESTED-OK] Seek bar clic souris (pas seulement clavier) — clic à ~60% de la barre saute correctement à la position correspondante.

## Reste à tester (pas encore couvert)

- [ ] Playlist : ajout multiple (+Ajouter), navigation N/P avec plusieurs éléments, Vider
- [ ] Drag & drop réel (fichier média + sous-titre) — mécanisme OS, difficile à automatiser ; code lu et cohérent (`handle_drop` dans app.rs), risque faible
- [ ] Fichiers récents (menu Fichier > Récents)
- [ ] Redimensionnement fenêtre / changement DPI
- [ ] Fermeture propre + relecture config au redémarrage (déjà vérifié que volume/vitesse sont sauvegardés en v1.4.0, pas revérifié cette session)
- [ ] Chargement sous-titre manuel (Fichier > Charger sous-titre…)
- [ ] Effacer sous-titre (Fichier > Effacer sous-titre)
- [ ] HDR — aucun fichier HDR disponible pour test
- [ ] Lecture réseau (HTTP/HLS) — non testé
- [ ] Recherche sous-titres OpenSubtitles / TMDB — nécessite clés API
- [ ] Sortie audio 5.1/7.1 réelle — aucun device surround disponible ici pour vérifier le fix v1.4.1 sur vrai matériel

---

## Journal chronologique
