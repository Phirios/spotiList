export type Me = {
  id: string;
  spotify_id: string;
  display_name: string | null;
  email: string | null;
  image_url: string | null;
};

export type LibraryItem = {
  id: string;
  name: string;
  artists: string[];
  album: string;
  image_url: string | null;
  added_at: string | null;
};

export type LibraryResponse = {
  items: LibraryItem[];
  total: number;
  limit: number;
  offset: number;
  q: string | null;
};

export type RankedTrack = {
  id: string;
  name: string;
  artists: string[];
  album: string;
  image_url: string | null;
  score: number;
};

export type GeneratedPlaylist = {
  vibe: string;
  model: string;
  considered: number;
  items: RankedTrack[];
};

export type SimilarTrack = {
  id: string;
  name: string;
  artists: string[];
  album: string;
  image_url: string | null;
  score: number;
};

export type TrackInfo = {
  id: string;
  name: string;
  artists: { id: string; name: string }[];
  album: {
    id: string;
    name: string;
    release_date: string | null;
    image_url: string | null;
  };
  duration_ms: number;
  explicit: boolean;
  popularity: number | null;
  isrc: string | null;
  spotify_url: string | null;
  genres: string[];
  bpm: { tempo: number; source: string } | null;
  lyrics: {
    plain: string | null;
    synced: string | null;
    instrumental: boolean;
    source: string;
  } | null;
};

export type Row = {
  id: string;
  name: string;
  artistsLine: string;
  album: string;
  image: string | null;
  trailing?: string;
};

export type SyncStatus = {
  status: "idle" | "running" | "done" | "failed";
  stage: "starting" | "fetching_library" | "fetching_tags" | "embedding" | null;
  progress: number;
  total: number;
  started_at: string | null;
  updated_at: string;
  finished_at: string | null;
  error: string | null;
};

export type AutoSummary = {
  id: string;
  name: string;
  description: string | null;
  track_count: number;
  spotify_playlist_id: string | null;
  created_at: string;
  sample: TrackOut[];
};

export type AutoPlaylistFull = {
  id: string;
  name: string;
  description: string | null;
  track_count: number;
  spotify_playlist_id: string | null;
  created_at: string;
  tracks: TrackOut[];
};

export type TrackOut = {
  id: string;
  name: string;
  artists: string[];
  album: string;
  image_url: string | null;
};

export const STAGE_LABEL: Record<NonNullable<SyncStatus["stage"]>, string> = {
  starting: "Starting up",
  fetching_library: "Fetching your liked songs",
  fetching_tags: "Looking up genres",
  embedding: "Computing embeddings",
};
