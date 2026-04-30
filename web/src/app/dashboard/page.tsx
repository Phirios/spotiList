"use client";

import { useEffect, useState } from "react";

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

export default function Dashboard() {
  const [me, setMe] = useState<Me | null>(null);
  const [liked, setLiked] = useState<LikedResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [vibe, setVibe] = useState("");
  const [generating, setGenerating] = useState(false);
  const [generated, setGenerated] = useState<GeneratedPlaylist | null>(null);
  const [generateError, setGenerateError] = useState<string | null>(null);

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

  async function handleGenerate(e: React.FormEvent) {
    e.preventDefault();
    if (!vibe.trim()) return;
    setGenerating(true);
    setGenerateError(null);
    setGenerated(null);
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

  async function logout() {
    await fetch("/api/auth/logout", { method: "POST", credentials: "include" });
    window.location.href = "/";
  }

  return (
    <div className="flex flex-1 flex-col bg-black font-sans text-zinc-200">
      <main className="flex flex-1 flex-col gap-12 px-6 py-20 sm:px-12 lg:px-20 max-w-5xl mx-auto w-full">
        <header className="flex flex-col gap-3">
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
          {generateError && (
            <p className="text-red-400 text-sm">{generateError}</p>
          )}
          {generated && (
            <div className="flex flex-col gap-2">
              <p className="text-zinc-500 text-xs">
                {generated.items.length} of {generated.considered} liked songs ·{" "}
                model: {generated.model.split("/").pop()}
              </p>
              <ul className="divide-y divide-zinc-900">
                {generated.items.map((t) => (
                  <li key={t.id} className="flex items-center gap-4 py-3">
                    {t.image_url && (
                      /* eslint-disable-next-line @next/next/no-img-element */
                      <img
                        src={t.image_url}
                        alt=""
                        className="h-12 w-12 rounded object-cover"
                      />
                    )}
                    <div className="flex-1 min-w-0">
                      <div className="text-white truncate">{t.name}</div>
                      <div className="text-sm text-zinc-400 truncate">
                        {t.artists.join(", ")} · {t.album}
                      </div>
                    </div>
                    <div className="text-xs text-zinc-500 hidden sm:block tabular-nums">
                      {t.score.toFixed(3)}
                    </div>
                  </li>
                ))}
              </ul>
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
            <ul className="divide-y divide-zinc-900">
              {liked.items.map(({ track, added_at }) => (
                <li key={track.id} className="flex items-center gap-4 py-3">
                  {track.album.images?.[0]?.url && (
                    /* eslint-disable-next-line @next/next/no-img-element */
                    <img
                      src={track.album.images[0].url}
                      alt=""
                      className="h-12 w-12 rounded object-cover"
                    />
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="text-white truncate">{track.name}</div>
                    <div className="text-sm text-zinc-400 truncate">
                      {track.artists.map((a) => a.name).join(", ")} ·{" "}
                      {track.album.name}
                    </div>
                  </div>
                  <div className="text-xs text-zinc-500 hidden sm:block">
                    {new Date(added_at).toLocaleDateString()}
                  </div>
                </li>
              ))}
            </ul>
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
