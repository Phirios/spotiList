# spoti

Vibe-based playlist generator over your Spotify liked songs. NLP coursework
project — describe a mood, get a playlist drawn from your library.

Live: https://spoti.phirios.com

## What it does

- Logs you in with Spotify OAuth, syncs your full liked-songs library
  in the background with progress + ETA.
- Enriches every track with Last.fm genre tags, GetSongBPM tempo, and
  LRCLIB lyrics (all cached in Postgres so each track only hits upstream
  once globally).
- Embeds each track with a local sentence-transformer (`all-MiniLM-L6-v2`,
  384-d) using `"<title> by <artists>. Tags: <last.fm tags>"`.
- **Vibe playlists** — type a free-form mood prompt; backend embeds it,
  cosine-sims against your library embeddings, returns top N. One click
  saves the result as a real Spotify playlist.
- **Smart playlists** — k-means over your embeddings, named by the most
  frequent Last.fm tags in each cluster. One click saves any cluster to
  Spotify.
- Click any track for a subpage with BPM, genres, lyrics, and **similar
  tracks in your library** (cosine sim within your own collection).
- Search bar + infinite scroll over the cached library.

## Stack

- **backend** — Rust, axum, sqlx (Postgres), reqwest. OAuth flow, library
  sync engine, vibe matcher, auto-clusterer.
- **embedder** — Python, FastAPI, sentence-transformers, scikit-learn.
  CPU-only PyTorch. Two endpoints: `/embed`, `/cluster`.
- **web** — Next.js 16 + React 19 + Tailwind 4. URL-routed track and
  playlist subpages, polling-based sync progress UI.
- **infra** — k8s namespace `nlp-project`, plain Postgres deployment,
  nginx ingress with cert-manager, manifests in
  [kubernetesmanifests](https://gitlab.enbitron.com/Phirios/kubernetesmanifests).

## Architecture

```
┌────────────────┐  OAuth  ┌────────────────┐
│ user (browser) │ ──────▶ │   spoti-web    │ ◀─ static landing + dashboard
└────────────────┘         └────────┬───────┘
                                    │ /api/* (Next rewrite)
                                    ▼
                           ┌────────────────┐    /embed    ┌─────────────────┐
                           │ spoti-backend  │ ───────────▶ │ spoti-embedder  │
                           │     (Rust)     │ ◀─ embeddings (sentence-       │
                           └────────┬───────┘                 transformers)  │
                            ┌───────┴───────┐                └────────────────┘
                            ▼               ▼
                       ┌─────────┐    ┌──────────┐
                       │postgres │    │ Spotify, │
                       │ (cache) │    │ Last.fm, │
                       └─────────┘    │ LRCLIB,  │
                                      │ GetSong- │
                                      │ BPM      │
                                      └──────────┘
```

## Why per-track caches are global

Tracks (`tracks`, `track_tags`, `track_embeddings`, `track_bpm`,
`track_lyrics`) are keyed by `spotify_track_id`, not per-user. The first
user who triggers enrichment for a track pays the upstream cost; every
subsequent user sees a cache hit. Only the `/me/tracks` enumeration
itself is per-user (Spotify gives no other way to know what's in a
specific user's library).

## Project layout

```
spotiList/
├── backend/        Rust axum service
│   ├── src/        modules: auth, sync, library, playlists, auto, …
│   ├── migrations/ sqlx-migrate SQL files
│   └── Dockerfile
├── embedder/       Python FastAPI sidecar (sentence-transformers + sklearn)
│   └── app/main.py
└── web/            Next.js dashboard
    └── src/app/dashboard/
        ├── page.tsx
        ├── components/  Column/section/panel components
        ├── types.ts
        └── dummy.ts     localhost-only mock data for layout previews
```

## Running locally

The web app has a `DUMMY` mode that activates when served from
`localhost` so you can preview the layout without backend access.

```sh
cd web
bun install
bun dev   # → http://localhost:3000/dashboard with mock data
```

For the full stack (backend + embedder + Postgres) you'll want the k8s
manifests or a `docker-compose` you assemble from the Dockerfiles. The
embedder pre-downloads the model into the image; first build is heavy
(~1.5 GB image with PyTorch CPU + model weights).

## Notes

- Spotify deprecated the **Audio Features**, **Audio Analysis**,
  **track popularity**, **artist genres**, **30s previews**, and
  **recommendations** endpoints for new apps in November 2024. We
  replace what we can: tempo via GetSongBPM (correct host
  `api.getsong.co`), genres via Last.fm tags, lyrics via LRCLIB.
- GetSongBPM has a 3000 req/hour limit. We cache hits **and** misses
  in `track_bpm` so we never re-query the same track.
- Last.fm tags are filtered to drop noise (year-only, decade,
  artist-name-equal tags) before being saved or used in embedding text.

## Data attributions

- BPM data: [GetSongBPM](https://getsongbpm.com)
- Lyrics: [LRCLIB](https://lrclib.net)
- Genre tags: [Last.fm](https://www.last.fm/api)

## License

Coursework project; source is here for reading. No license attached
yet — assume "all rights reserved" and ask if you want to use anything.
