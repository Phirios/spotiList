"use client";

import { Suspense, useCallback, useEffect, useState } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";

import { LeftColumn } from "./components/LeftColumn";
import { LibrarySection } from "./components/LibrarySection";
import { PlaylistPanel } from "./components/PlaylistPanel";
import { SmartPlaylists } from "./components/SmartPlaylists";
import { TrackPanel } from "./components/TrackPanel";
import {
  DUMMY,
  DUMMY_TRACKS,
  dummyAutos,
  dummyImage,
  dummyLibraryPage,
  dummyPlaylistFull,
  dummySimilar,
  dummyTrackInfo,
} from "./dummy";
import type {
  AutoPlaylistFull,
  AutoSummary,
  GeneratedPlaylist,
  LibraryResponse,
  Me,
  Row,
  SimilarTrack,
  SyncStatus,
  TrackInfo,
} from "./types";

export default function DashboardPage() {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen bg-black font-sans text-zinc-200 flex items-center justify-center">
          <span className="text-zinc-500 text-sm">Loading…</span>
        </div>
      }
    >
      <Dashboard />
    </Suspense>
  );
}

function Dashboard() {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const selectedTrack = searchParams.get("track");
  const selectedPlaylist = searchParams.get("playlist");

  // Update one or more URL search params, removing keys explicitly set to null.
  const setParams = useCallback(
    (updates: Record<string, string | null>) => {
      const params = new URLSearchParams(searchParams.toString());
      for (const [k, v] of Object.entries(updates)) {
        if (v === null) params.delete(k);
        else params.set(k, v);
      }
      const qs = params.toString();
      router.push(qs ? `${pathname}?${qs}` : pathname);
    },
    [router, pathname, searchParams],
  );

  const [me, setMe] = useState<Me | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const [library, setLibrary] = useState<LibraryResponse | null>(null);
  const [libraryQ, setLibraryQ] = useState("");
  const [libraryLoading, setLibraryLoading] = useState(false);
  const [libraryMoreLoading, setLibraryMoreLoading] = useState(false);

  const [vibe, setVibe] = useState("");
  const [generating, setGenerating] = useState(false);
  const [generated, setGenerated] = useState<GeneratedPlaylist | null>(null);
  const [generateError, setGenerateError] = useState<string | null>(null);

  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<{ url: string; name: string } | null>(
    null,
  );
  const [saveError, setSaveError] = useState<string | null>(null);

  const [autos, setAutos] = useState<AutoSummary[] | null>(null);
  const [autosLoading, setAutosLoading] = useState(true);
  const [regenerating, setRegenerating] = useState(false);
  const [autoError, setAutoError] = useState<string | null>(null);
  const [autoSaving, setAutoSaving] = useState<string | null>(null);
  const [autoSaved, setAutoSaved] = useState<Record<string, string>>({});

  const [sync, setSync] = useState<SyncStatus | null>(null);
  const isSyncing = sync?.status === "running";
  const isSyncDone = sync?.status === "done";

  const [details, setDetails] = useState<Record<string, TrackInfo>>({});
  const [detailLoading, setDetailLoading] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [similar, setSimilar] = useState<Record<string, SimilarTrack[]>>({});
  const [similarLoading, setSimilarLoading] = useState<string | null>(null);

  const [playlistFull, setPlaylistFull] = useState<
    Record<string, AutoPlaylistFull>
  >({});
  const [playlistLoading, setPlaylistLoading] = useState<string | null>(null);
  const [playlistError, setPlaylistError] = useState<string | null>(null);

  // Initial load
  useEffect(() => {
    if (DUMMY) {
      setMe({
        id: "u1",
        spotify_id: "phirios",
        display_name: "Phirios",
        email: "kirazh27@gmail.com",
        image_url: "https://picsum.photos/seed/phirios-avatar/200",
      });
      setLibrary(dummyLibraryPage(0, 30, null));
      setAutos(dummyAutos());
      setSync({
        status: "done",
        stage: null,
        progress: 0,
        total: 0,
        started_at: null,
        updated_at: new Date().toISOString(),
        finished_at: new Date(Date.now() - 5 * 60_000).toISOString(),
        error: null,
      });
      setLoading(false);
      setAutosLoading(false);
      return;
    }
    (async () => {
      try {
        const meRes = await fetch("/api/auth/me", { credentials: "include" });
        if (meRes.status === 401 || meRes.status === 403) {
          window.location.href = "/api/auth/login";
          return;
        }
        if (!meRes.ok) throw new Error(`me ${meRes.status}`);
        setMe(await meRes.json());

        const libRes = await fetch("/api/me/library?limit=30", {
          credentials: "include",
        });
        if (!libRes.ok) throw new Error(`library ${libRes.status}`);
        setLibrary(await libRes.json());
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }

      try {
        const r = await fetch("/api/auto-playlists", {
          credentials: "include",
        });
        if (r.ok) setAutos(await r.json());
      } catch {
        // ignore — surface on regenerate
      } finally {
        setAutosLoading(false);
      }
    })();
  }, []);

  // Sync polling. While running, poll fast; otherwise slow.
  useEffect(() => {
    if (DUMMY) return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const poll = async () => {
      try {
        const r = await fetch("/api/library/sync", { credentials: "include" });
        if (!cancelled && r.ok) setSync((await r.json()) as SyncStatus);
      } catch {
        // ignore
      }
      if (!cancelled) {
        const wait = sync?.status === "running" ? 1500 : 30000;
        timer = setTimeout(poll, wait);
      }
    };
    poll();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sync?.status]);

  // Auto-trigger sync if status is idle (existing users / first load).
  useEffect(() => {
    if (sync?.status === "idle") startSync(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sync?.status]);

  // Debounced library search.
  useEffect(() => {
    if (loading) return;
    if (DUMMY) {
      const q = libraryQ.trim() || null;
      setLibrary(dummyLibraryPage(0, 30, q));
      return;
    }
    const t = setTimeout(async () => {
      setLibraryLoading(true);
      try {
        const url = libraryQ.trim()
          ? `/api/me/library?limit=50&q=${encodeURIComponent(libraryQ.trim())}`
          : `/api/me/library?limit=30`;
        const r = await fetch(url, { credentials: "include" });
        if (r.ok) setLibrary(await r.json());
      } finally {
        setLibraryLoading(false);
      }
    }, 250);
    return () => clearTimeout(t);
  }, [libraryQ, loading]);

  // Fetch track detail + similar when URL changes.
  useEffect(() => {
    if (!selectedTrack) return;
    setDetailError(null);

    if (DUMMY) {
      setDetails((prev) =>
        prev[selectedTrack]
          ? prev
          : { ...prev, [selectedTrack]: dummyTrackInfo(selectedTrack) },
      );
      setSimilar((prev) =>
        prev[selectedTrack]
          ? prev
          : { ...prev, [selectedTrack]: dummySimilar(selectedTrack) },
      );
      return;
    }

    let cancelled = false;
    if (!details[selectedTrack]) {
      setDetailLoading(selectedTrack);
      (async () => {
        try {
          const r = await fetch(`/api/tracks/${selectedTrack}`);
          if (!r.ok) throw new Error(`${r.status}`);
          const info = (await r.json()) as TrackInfo;
          if (!cancelled) {
            setDetails((prev) => ({ ...prev, [selectedTrack]: info }));
          }
        } catch (e) {
          if (!cancelled) setDetailError(String(e));
        } finally {
          if (!cancelled) setDetailLoading(null);
        }
      })();
    }
    if (!similar[selectedTrack]) {
      setSimilarLoading(selectedTrack);
      (async () => {
        try {
          const r = await fetch(
            `/api/tracks/${selectedTrack}/similar?limit=8`,
            { credentials: "include" },
          );
          if (r.ok) {
            const list = (await r.json()) as SimilarTrack[];
            if (!cancelled) {
              setSimilar((prev) => ({ ...prev, [selectedTrack]: list }));
            }
          }
        } finally {
          if (!cancelled) setSimilarLoading(null);
        }
      })();
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedTrack]);

  // Fetch playlist detail when ?playlist changes (cached per id).
  useEffect(() => {
    if (!selectedPlaylist) return;
    setPlaylistError(null);
    if (DUMMY) {
      setPlaylistFull((prev) =>
        prev[selectedPlaylist]
          ? prev
          : { ...prev, [selectedPlaylist]: dummyPlaylistFull(selectedPlaylist) },
      );
      return;
    }
    if (playlistFull[selectedPlaylist]) return;
    let cancelled = false;
    setPlaylistLoading(selectedPlaylist);
    (async () => {
      try {
        const r = await fetch(`/api/auto-playlists/${selectedPlaylist}`, {
          credentials: "include",
        });
        if (!r.ok) throw new Error(`${r.status}`);
        const data = (await r.json()) as AutoPlaylistFull;
        if (!cancelled) {
          setPlaylistFull((prev) => ({ ...prev, [selectedPlaylist]: data }));
        }
      } catch (e) {
        if (!cancelled) setPlaylistError(String(e));
      } finally {
        if (!cancelled) setPlaylistLoading(null);
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedPlaylist]);

  const selectTrack = useCallback(
    (id: string) => setParams({ track: id }),
    [setParams],
  );
  const closeTrack = useCallback(() => setParams({ track: null }), [setParams]);
  const selectPlaylist = useCallback(
    (id: string) => setParams({ playlist: id, track: null }),
    [setParams],
  );
  const closePlaylist = useCallback(
    () => setParams({ playlist: null }),
    [setParams],
  );

  const loadMoreLibrary = useCallback(async () => {
    if (libraryMoreLoading || !library) return;
    if (library.items.length >= library.total) return;
    setLibraryMoreLoading(true);
    try {
      if (DUMMY) {
        await new Promise((r) => setTimeout(r, 250));
        const next = dummyLibraryPage(library.items.length, 30, library.q);
        setLibrary({ ...next, items: [...library.items, ...next.items] });
        return;
      }
      const params = new URLSearchParams({
        limit: "50",
        offset: String(library.items.length),
      });
      if (library.q) params.set("q", library.q);
      const r = await fetch(`/api/me/library?${params}`, {
        credentials: "include",
      });
      if (r.ok) {
        const next = (await r.json()) as LibraryResponse;
        setLibrary({ ...next, items: [...library.items, ...next.items] });
      }
    } finally {
      setLibraryMoreLoading(false);
    }
  }, [library, libraryMoreLoading]);

  const onLibraryScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const el = e.currentTarget;
      if (el.scrollHeight - el.scrollTop - el.clientHeight < 300) {
        loadMoreLibrary();
      }
    },
    [loadMoreLibrary],
  );

  async function startSync(force = false) {
    try {
      const r = await fetch("/api/library/sync", {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ force }),
      });
      if (r.ok) setSync(await r.json());
    } catch {
      // ignore
    }
  }

  async function handleGenerate(e: React.FormEvent) {
    e.preventDefault();
    if (!vibe.trim()) return;
    setGenerating(true);
    setGenerateError(null);
    setGenerated(null);
    setSaved(null);
    setSaveError(null);
    if (DUMMY) {
      await new Promise((r) => setTimeout(r, 600));
      setGenerated({
        vibe,
        model: "sentence-transformers/all-MiniLM-L6-v2#v3-cache",
        considered: 2873,
        items: DUMMY_TRACKS.slice(0, 12).map((t, i) => ({
          id: t.id,
          name: t.name,
          artists: t.artists,
          album: t.album,
          image_url: dummyImage(t.id),
          score: 0.95 - i * 0.04,
        })),
      });
      setGenerating(false);
      return;
    }
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
    if (DUMMY) {
      await new Promise((r) => setTimeout(r, 400));
      setSaved({
        url: "https://open.spotify.com/playlist/dummyid",
        name:
          generated.vibe.length > 60
            ? generated.vibe.slice(0, 57) + "…"
            : generated.vibe,
      });
      setSaving(false);
      return;
    }
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

  async function handleRegenerate() {
    setRegenerating(true);
    setAutoError(null);
    if (DUMMY) {
      await new Promise((r) => setTimeout(r, 800));
      setAutos(dummyAutos());
      setAutoSaved({});
      setRegenerating(false);
      return;
    }
    try {
      const r = await fetch("/api/auto-playlists/regenerate", {
        method: "POST",
        credentials: "include",
      });
      if (!r.ok) {
        const text = await r.text();
        throw new Error(`${r.status}: ${text}`);
      }
      const data = (await r.json()) as { playlists: AutoSummary[] };
      setAutos(data.playlists);
      setAutoSaved({});
    } catch (e) {
      setAutoError(String(e));
    } finally {
      setRegenerating(false);
    }
  }

  async function handleSaveAuto(p: AutoSummary) {
    setAutoSaving(p.id);
    if (DUMMY) {
      await new Promise((r) => setTimeout(r, 400));
      setAutoSaved((prev) => ({
        ...prev,
        [p.id]: "https://open.spotify.com/playlist/dummyid",
      }));
      setAutoSaving(null);
      return;
    }
    try {
      const r = await fetch(`/api/auto-playlists/${p.id}/save`, {
        method: "POST",
        credentials: "include",
      });
      if (!r.ok) {
        const text = await r.text();
        throw new Error(`${r.status}: ${text}`);
      }
      const data = (await r.json()) as { url: string };
      setAutoSaved((prev) => ({ ...prev, [p.id]: data.url }));
    } catch (e) {
      setAutoError(String(e));
    } finally {
      setAutoSaving(null);
    }
  }

  async function logout() {
    await fetch("/api/auth/logout", {
      method: "POST",
      credentials: "include",
    });
    window.location.href = "/";
  }

  const libraryRows: Row[] =
    library?.items.map((t) => ({
      id: t.id,
      name: t.name,
      artistsLine: t.artists.join(", "),
      album: t.album,
      image: t.image_url,
      trailing: t.added_at
        ? new Date(t.added_at).toLocaleDateString()
        : undefined,
    })) ?? [];

  return (
    <div className="min-h-screen bg-black font-sans text-zinc-200">
      <div className="mx-auto w-full max-w-[1600px] px-4 sm:px-6 py-6 grid gap-6 grid-cols-1 lg:grid-cols-[320px_minmax(0,1fr)_360px] lg:h-screen lg:overflow-hidden">
        <LeftColumn
          me={me}
          error={error}
          sync={sync}
          onResync={() => startSync(true)}
          vibe={vibe}
          setVibe={setVibe}
          generating={generating}
          generated={generated}
          generateError={generateError}
          saving={saving}
          saved={saved}
          saveError={saveError}
          isSyncing={isSyncing}
          isSyncDone={isSyncDone}
          selectedTrack={selectedTrack}
          onGenerate={handleGenerate}
          onSave={handleSave}
          onSelectTrack={selectTrack}
          onLogout={logout}
        />

        <main className="min-w-0 flex flex-col gap-5 min-h-0 lg:max-h-[calc(100vh-3rem)]">
          {selectedTrack ? (
            <TrackPanel
              loading={detailLoading === selectedTrack}
              error={detailError}
              info={details[selectedTrack]}
              similar={similar[selectedTrack]}
              similarLoading={similarLoading === selectedTrack}
              onClose={closeTrack}
              onPick={selectTrack}
            />
          ) : selectedPlaylist ? (
            <PlaylistPanel
              loading={playlistLoading === selectedPlaylist}
              error={playlistError}
              playlist={playlistFull[selectedPlaylist]}
              saving={autoSaving === selectedPlaylist}
              savedUrl={autoSaved[selectedPlaylist] ?? null}
              onSave={() => {
                const summary = autos?.find((a) => a.id === selectedPlaylist);
                if (summary) handleSaveAuto(summary);
              }}
              onClose={closePlaylist}
              onPickTrack={selectTrack}
            />
          ) : (
            <LibrarySection
              library={library}
              libraryQ={libraryQ}
              setLibraryQ={setLibraryQ}
              libraryLoading={libraryLoading}
              libraryMoreLoading={libraryMoreLoading}
              onScroll={onLibraryScroll}
              libraryRows={libraryRows}
              selectedTrack={selectedTrack}
              onSelectTrack={selectTrack}
            />
          )}
        </main>

        <SmartPlaylists
          autos={autos}
          autosLoading={autosLoading}
          regenerating={regenerating}
          autoError={autoError}
          autoSaving={autoSaving}
          autoSaved={autoSaved}
          isSyncing={isSyncing}
          isSyncDone={isSyncDone}
          onRegenerate={handleRegenerate}
          onOpen={selectPlaylist}
          onSave={handleSaveAuto}
          onPickTrack={selectTrack}
        />
      </div>
    </div>
  );
}
