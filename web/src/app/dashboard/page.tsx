"use client";

import { useEffect, useState, useCallback } from "react";

type Me = {
  id: string;
  spotify_id: string;
  display_name: string | null;
  email: string | null;
};

type LikedItem = {
  added_at: string;
  track: {
    id: string;
    name: string;
    duration_ms: number;
    artists: { id: string; name: string }[];
    album: { name: string; images?: { url: string }[] };
  };
};

type LikedResponse = {
  items: LikedItem[];
  total: number;
  limit: number;
  offset: number;
};

type RankedTrack = {
  id: string;
  name: string;
  artists: string[];
  album: string;
  image_url: string | null;
  score: number;
};

type GeneratedPlaylist = {
  vibe: string;
  model: string;
  considered: number;
  items: RankedTrack[];
};

type TrackInfo = {
  id: string;
  name: string;
  artists: { id: string; name: string }[];
  album: { id: string; name: string; release_date: string | null; image_url: string | null };
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

type Row = {
  id: string;
  name: string;
  artistsLine: string;
  album: string;
  image: string | null;
  trailing?: string;
};

export default function Dashboard() {
  const [me, setMe] = useState<Me | null>(null);
  const [liked, setLiked] = useState<LikedResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [vibe, setVibe] = useState("");
  const [generating, setGenerating] = useState(false);
  const [generated, setGenerated] = useState<GeneratedPlaylist | null>(null);
  const [generateError, setGenerateError] = useState<string | null>(null);

  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ url: string; name: string } | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [expanded, setExpanded] = useState<string | null>(null);
  const [details, setDetails] = useState<Record<string, TrackInfo>>({});
  const [detailLoading, setDetailLoading] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const meRes = await fetch("/api/auth/me", { credentials: "include" });
        if (meRes.status === 401 || meRes.status === 403) {
          window.location.href = "/api/auth/login";
          return;
        }
        if (!meRes.ok) throw new Error(`me ${meRes.status}`);
        setMe(await meRes.json());

        const likedRes = await fetch("/api/me/liked?limit=20", { credentials: "include" });
        if (!likedRes.ok) throw new Error(`liked ${likedRes.status}`);
        setLiked(await likedRes.json());
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const toggleTrack = useCallback(
    async (id: string) => {
      if (expanded === id) {
        setExpanded(null);
        return;
      }
      setExpanded(id);
      setDetailError(null);
      if (details[id]) return;
      setDetailLoading(id);
      try {
        const r = await fetch(`/api/tracks/${id}`);
        if (!r.ok) throw new Error(`${r.status}`);
        const info = (await r.json()) as TrackInfo;
        setDetails((prev) => ({ ...prev, [id]: info }));
      } catch (e) {
        setDetailError(String(e));
      } finally {
        setDetailLoading(null);
      }
    },
    [expanded, details],
  );

  async function handleGenerate(e: React.FormEvent) {
    e.preventDefault();
    if (!vibe.trim()) return;
    setGenerating(true);
    setGenerateError(null);
    setGenerated(null);
    setSaved(null);
    setSaveError(null);
    try {
      const r = await fetch("/api/playlists/generate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ vibe, limit: 20 }),
      });
      if (!r.ok) {
        const text = await r.text();
        throw new Error(`${r.status}: ${text}`);
      }
      setGenerated(await r.json());
    } catch (err) {
      setGenerateError(String(err));
    } finally {
      setGenerating(false);
    }
  }

  async function handleSave() {
    if (!generated || generated.items.length === 0) return;
    setSaving(true);
    setSaveError(null);
    setSaved(null);
    try {
      const name =
        generated.vibe.length > 60
          ? generated.vibe.slice(0, 57) + "…"
          : generated.vibe;
      const r = await fetch("/api/playlists/save", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          name,
          vibe: generated.vibe,
          track_ids: generated.items.map((t) => t.id),
          public: false,
        }),
      });
      if (!r.ok) {
        const text = await r.text();
        throw new Error(`${r.status}: ${text}`);
      }
      const data = (await r.json()) as { url: string; name: string };
      setSaved({ url: data.url, name: data.name });
    } catch (err) {
      setSaveError(String(err));
    } finally {
      setSaving(false);
    }
  }

  async function logout() {
    await fetch("/api/auth/logout", { method: "POST", credentials: "include" });
    window.location.href = "/";
  }

  const likedRows: Row[] =
    liked?.items.map(({ track, added_at }) => ({
      id: track.id,
      name: track.name,
      artistsLine: track.artists.map((a) => a.name).join(", "),
      album: track.album.name,
      image: track.album.images?.[0]?.url ?? null,
      trailing: new Date(added_at).toLocaleDateString(),
    })) ?? [];

  const generatedRows: Row[] =
    generated?.items.map((t) => ({
      id: t.id,
      name: t.name,
      artistsLine: t.artists.join(", "),
      album: t.album,
      image: t.image_url,
      trailing: t.score.toFixed(3),
    })) ?? [];

  return (
    <div className="flex flex-1 flex-col bg-black font-sans text-zinc-200">
      <main className="flex flex-1 flex-col gap-12 px-6 py-20 sm:px-12 lg:px-20 max-w-5xl mx-auto w-full">
        <header className="flex flex-col gap-3">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src="/logo.png"
            alt="spoti"
            className="h-12 w-12 rounded-xl mb-2"
          />
          <span className="text-sm uppercase tracking-[0.3em] text-emerald-400">
            dashboard
          </span>
          <h1 className="text-3xl sm:text-5xl font-semibold tracking-tight text-white">
            {loading
              ? "Loading…"
              : me
                ? `Hey ${me.display_name ?? me.spotify_id}`
                : "Not logged in"}
          </h1>
          {error && <p className="text-red-400 text-sm">{error}</p>}
        </header>

        <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-8 sm:p-10 flex flex-col gap-5">
          <div>
            <h2 className="text-xl font-semibold text-white mb-1">
              Generate a vibe playlist
            </h2>
            <p className="text-zinc-400 text-sm">
              Describe a mood. We&rsquo;ll search your liked songs.
            </p>
          </div>
          <form onSubmit={handleGenerate} className="flex flex-col sm:flex-row gap-3">
            <input
              type="text"
              value={vibe}
              onChange={(e) => setVibe(e.target.value)}
              placeholder="late-night drive after a long week"
              className="flex-1 rounded-full bg-black border border-zinc-800 px-5 py-3 text-white placeholder:text-zinc-600 focus:outline-none focus:border-emerald-500"
            />
            <button
              type="submit"
              disabled={generating || !vibe.trim()}
              className="rounded-full bg-emerald-500 px-6 py-3 text-sm font-semibold text-black hover:bg-emerald-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {generating ? "Generating…" : "Generate"}
            </button>
          </form>
          {generateError && <p className="text-red-400 text-sm">{generateError}</p>}
          {generated && (
            <div className="flex flex-col gap-3">
              <div className="flex items-center justify-between gap-3 flex-wrap">
                <p className="text-zinc-500 text-xs">
                  {generated.items.length} of {generated.considered} liked songs ·{" "}
                  model: {generated.model.split("/").pop()}
                </p>
                <div className="flex items-center gap-3">
                  {saved ? (
                    <a
                      href={saved.url}
                      target="_blank"
                      rel="noopener"
                      className="rounded-full bg-emerald-500 px-4 py-1.5 text-xs font-semibold text-black hover:bg-emerald-400"
                    >
                      Open &ldquo;{saved.name}&rdquo; on Spotify ↗
                    </a>
                  ) : (
                    <button
                      type="button"
                      onClick={handleSave}
                      disabled={saving || generated.items.length === 0}
                      className="rounded-full border border-emerald-500 text-emerald-400 px-4 py-1.5 text-xs font-semibold hover:bg-emerald-500/10 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                    >
                      {saving ? "Saving…" : "Save to Spotify"}
                    </button>
                  )}
                </div>
              </div>
              {saveError && <p className="text-red-400 text-xs">{saveError}</p>}
              <TrackList
                rows={generatedRows}
                expanded={expanded}
                details={details}
                detailLoading={detailLoading}
                detailError={detailError}
                onToggle={toggleTrack}
              />
            </div>
          )}
        </section>

        {liked && (
          <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-8 sm:p-10 flex flex-col gap-6">
            <div>
              <h2 className="text-xl font-semibold text-white mb-1">
                Your library
              </h2>
              <p className="text-zinc-400 text-sm">
                {liked.total.toLocaleString()} liked songs · showing{" "}
                {liked.items.length}
              </p>
            </div>
            <TrackList
              rows={likedRows}
              expanded={expanded}
              details={details}
              detailLoading={detailLoading}
              detailError={detailError}
              onToggle={toggleTrack}
            />
          </section>
        )}

        {me && (
          <section className="flex gap-3">
            <button
              onClick={logout}
              className="rounded-full border border-zinc-700 px-5 py-2 text-sm hover:bg-zinc-900"
            >
              Log out
            </button>
          </section>
        )}
      </main>
    </div>
  );
}

function TrackList({
  rows,
  expanded,
  details,
  detailLoading,
  detailError,
  onToggle,
}: {
  rows: Row[];
  expanded: string | null;
  details: Record<string, TrackInfo>;
  detailLoading: string | null;
  detailError: string | null;
  onToggle: (id: string) => void;
}) {
  return (
    <ul className="divide-y divide-zinc-900">
      {rows.map((row) => {
        const isOpen = expanded === row.id;
        const info = details[row.id];
        return (
          <li key={row.id} className="flex flex-col">
            <button
              type="button"
              onClick={() => onToggle(row.id)}
              className="flex items-center gap-4 py-3 text-left w-full hover:bg-zinc-900/50 -mx-3 px-3 rounded-md transition-colors"
            >
              {row.image && (
                /* eslint-disable-next-line @next/next/no-img-element */
                <img
                  src={row.image}
                  alt=""
                  className="h-12 w-12 rounded object-cover flex-shrink-0"
                />
              )}
              <div className="flex-1 min-w-0">
                <div className="text-white truncate">{row.name}</div>
                <div className="text-sm text-zinc-400 truncate">
                  {row.artistsLine} · {row.album}
                </div>
              </div>
              {row.trailing && (
                <div className="text-xs text-zinc-500 hidden sm:block tabular-nums flex-shrink-0">
                  {row.trailing}
                </div>
              )}
            </button>
            {isOpen && (
              <TrackDetail
                loading={detailLoading === row.id}
                error={detailError}
                info={info}
              />
            )}
          </li>
        );
      })}
    </ul>
  );
}

function TrackDetail({
  loading,
  error,
  info,
}: {
  loading: boolean;
  error: string | null;
  info: TrackInfo | undefined;
}) {
  return (
    <div className="ml-16 mr-3 my-3 rounded-xl border border-zinc-800 bg-black/40 p-5 text-sm">
      {loading && <p className="text-zinc-500">Loading metadata…</p>}
      {error && !loading && <p className="text-red-400">{error}</p>}
      {info && (
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-5">
          <Field label="BPM">
            {info.bpm ? (
              <span className="text-white tabular-nums">
                {info.bpm.tempo.toFixed(1)}{" "}
                <span className="text-zinc-500 text-xs">
                  ({info.bpm.source})
                </span>
              </span>
            ) : (
              <span className="text-zinc-600">—</span>
            )}
          </Field>
          <Field label="Genres">
            {info.genres.length > 0 ? (
              <div className="flex flex-wrap gap-1.5">
                {info.genres.map((g) => (
                  <span
                    key={g}
                    className="rounded-full bg-zinc-800 px-2.5 py-0.5 text-xs text-zinc-300"
                  >
                    {g}
                  </span>
                ))}
              </div>
            ) : (
              <span className="text-zinc-600">—</span>
            )}
          </Field>
          <Field label="Popularity">
            {info.popularity !== null ? (
              <span className="text-white tabular-nums">{info.popularity}</span>
            ) : (
              <span className="text-zinc-600">—</span>
            )}
          </Field>
          <div className="sm:col-span-3">
            <div className="text-xs uppercase tracking-wider text-zinc-500 mb-2">
              Lyrics
            </div>
            {info.lyrics?.synced ? (
              <pre className="whitespace-pre-wrap font-sans text-zinc-300 max-h-64 overflow-y-auto">
                {info.lyrics.synced}
              </pre>
            ) : info.lyrics?.plain ? (
              <pre className="whitespace-pre-wrap font-sans text-zinc-300 max-h-64 overflow-y-auto">
                {info.lyrics.plain}
              </pre>
            ) : info.lyrics?.instrumental ? (
              <p className="text-zinc-500">Instrumental</p>
            ) : (
              <p className="text-zinc-600">—</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-xs uppercase tracking-wider text-zinc-500 mb-1">{label}</div>
      <div>{children}</div>
    </div>
  );
}
