import type {
  AutoPlaylistFull,
  AutoSummary,
  LibraryItem,
  LibraryResponse,
  SimilarTrack,
  TrackInfo,
} from "./types";

// On localhost we can't authenticate (cookies are scoped to spoti.phirios.com),
// so we render with dummy data to make the layout previewable. This entire
// branch is gated behind a hostname check so production never sees it.
export const DUMMY =
  typeof window !== "undefined" && window.location.hostname === "localhost";

export const DUMMY_TRACKS = [
  { id: "t1", name: "Shape of You", artists: ["Ed Sheeran"], album: "÷" },
  { id: "t2", name: "Bohemian Rhapsody", artists: ["Queen"], album: "A Night at the Opera" },
  { id: "t3", name: "Blinding Lights", artists: ["The Weeknd"], album: "After Hours" },
  { id: "t4", name: "Heat Waves", artists: ["Glass Animals"], album: "Dreamland" },
  { id: "t5", name: "As It Was", artists: ["Harry Styles"], album: "Harry's House" },
  { id: "t6", name: "Anti-Hero", artists: ["Taylor Swift"], album: "Midnights" },
  { id: "t7", name: "Flowers", artists: ["Miley Cyrus"], album: "Endless Summer Vacation" },
  { id: "t8", name: "Cruel Summer", artists: ["Taylor Swift"], album: "Lover" },
  { id: "t9", name: "Late Night Talking", artists: ["Harry Styles"], album: "Harry's House" },
  { id: "t10", name: "Watermelon Sugar", artists: ["Harry Styles"], album: "Fine Line" },
  { id: "t11", name: "drivers license", artists: ["Olivia Rodrigo"], album: "SOUR" },
  { id: "t12", name: "good 4 u", artists: ["Olivia Rodrigo"], album: "SOUR" },
  { id: "t13", name: "Stay", artists: ["The Kid LAROI", "Justin Bieber"], album: "F*CK LOVE 3" },
  { id: "t14", name: "Easy On Me", artists: ["Adele"], album: "30" },
  { id: "t15", name: "Bad Habit", artists: ["Steve Lacy"], album: "Gemini Rights" },
  { id: "t16", name: "About Damn Time", artists: ["Lizzo"], album: "Special" },
  { id: "t17", name: "Running Up That Hill", artists: ["Kate Bush"], album: "Hounds of Love" },
  { id: "t18", name: "Unholy", artists: ["Sam Smith", "Kim Petras"], album: "Gloria" },
  { id: "t19", name: "Levitating", artists: ["Dua Lipa"], album: "Future Nostalgia" },
  { id: "t20", name: "Industry Baby", artists: ["Lil Nas X", "Jack Harlow"], album: "MONTERO" },
];

export const DUMMY_TOTAL = 2873;

export const dummyImage = (id: string) =>
  `https://picsum.photos/seed/${id}/200`;

export const dummyLibraryPage = (
  offset: number,
  limit: number,
  q: string | null,
): LibraryResponse => {
  if (q) {
    const filtered = DUMMY_TRACKS.filter(
      (t) =>
        t.name.toLowerCase().includes(q.toLowerCase()) ||
        t.album.toLowerCase().includes(q.toLowerCase()) ||
        t.artists.some((a) => a.toLowerCase().includes(q.toLowerCase())),
    );
    return {
      total: filtered.length,
      limit,
      offset,
      q,
      items: filtered.slice(offset, offset + limit).map((t, i) => ({
        id: `${t.id}-${offset + i}`,
        name: t.name,
        artists: t.artists,
        album: t.album,
        image_url: dummyImage(t.id),
        added_at: new Date(Date.now() - (offset + i) * 1e8).toISOString(),
      })),
    };
  }
  const items: LibraryItem[] = [];
  for (let i = offset; i < Math.min(offset + limit, DUMMY_TOTAL); i++) {
    const t = DUMMY_TRACKS[i % DUMMY_TRACKS.length];
    items.push({
      id: `${t.id}-${i}`,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(`${t.id}-${i}`),
      added_at: new Date(Date.now() - i * 1e8).toISOString(),
    });
  }
  return { total: DUMMY_TOTAL, limit, offset, q: null, items };
};

export const dummyAutos = (): AutoSummary[] => [
  {
    id: "a1",
    name: "indie · pop · upbeat",
    description: "284 tracks · auto-clustered from your liked songs",
    track_count: 284,
    spotify_playlist_id: null,
    created_at: new Date().toISOString(),
    sample: DUMMY_TRACKS.slice(0, 4).map((t) => ({
      id: t.id,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(t.id),
    })),
  },
  {
    id: "a2",
    name: "rock · classic · epic",
    description: "162 tracks · auto-clustered from your liked songs",
    track_count: 162,
    spotify_playlist_id: null,
    created_at: new Date().toISOString(),
    sample: DUMMY_TRACKS.slice(4, 8).map((t) => ({
      id: t.id,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(t.id),
    })),
  },
  {
    id: "a3",
    name: "synthwave · dance · electronic",
    description: "211 tracks · auto-clustered from your liked songs",
    track_count: 211,
    spotify_playlist_id: null,
    created_at: new Date().toISOString(),
    sample: DUMMY_TRACKS.slice(8, 12).map((t) => ({
      id: t.id,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(t.id),
    })),
  },
  {
    id: "a4",
    name: "chill · acoustic · singer-songwriter",
    description: "147 tracks · auto-clustered from your liked songs",
    track_count: 147,
    spotify_playlist_id: null,
    created_at: new Date().toISOString(),
    sample: DUMMY_TRACKS.slice(12, 16).map((t) => ({
      id: t.id,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(t.id),
    })),
  },
];

export const dummySimilar = (id: string): SimilarTrack[] =>
  DUMMY_TRACKS.filter((t) => t.id !== id)
    .slice(0, 6)
    .map((t, i) => ({
      id: t.id,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(t.id),
      score: 0.92 - i * 0.07,
    }));

export const dummyPlaylistFull = (id: string): AutoPlaylistFull => {
  const meta = dummyAutos().find((a) => a.id === id) ?? dummyAutos()[0];
  // Synthesize ~50 tracks by cycling through DUMMY_TRACKS.
  const count = 32 + ((id.charCodeAt(0) ?? 0) % 30);
  const tracks = Array.from({ length: count }).map((_, i) => {
    const t = DUMMY_TRACKS[i % DUMMY_TRACKS.length];
    return {
      id: `${t.id}-${id}-${i}`,
      name: t.name,
      artists: t.artists,
      album: t.album,
      image_url: dummyImage(`${t.id}-${id}-${i}`),
    };
  });
  return {
    id: meta.id,
    name: meta.name,
    description: meta.description,
    track_count: count,
    spotify_playlist_id: null,
    created_at: meta.created_at,
    tracks,
  };
};

export const dummyTrackInfo = (id: string): TrackInfo => {
  const t = DUMMY_TRACKS.find((x) => x.id === id) ?? DUMMY_TRACKS[0];
  return {
    id: t.id,
    name: t.name,
    artists: t.artists.map((n) => ({ id: "", name: n })),
    album: {
      id: "",
      name: t.album,
      release_date: "2023-01-01",
      image_url: dummyImage(t.id),
    },
    duration_ms: 210_000,
    explicit: false,
    popularity: null,
    isrc: null,
    spotify_url: `https://open.spotify.com/track/${t.id}`,
    genres: ["pop", "indie pop", "alternative"],
    bpm: { tempo: 120 + Math.floor(Math.random() * 40), source: "getsongbpm" },
    lyrics: {
      plain:
        "I see your face in my mind as I drive away\n'Cause none of us thought it was gonna end that way\nPeople are people and sometimes we change our minds\nBut it's killing me to see you go after all this time",
      synced: null,
      instrumental: false,
      source: "lrclib",
    },
  };
};
