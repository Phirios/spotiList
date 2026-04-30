"use client";

import { useEffect } from "react";
import type { AutoPlaylistFull } from "../types";

export function PlaylistPanel({
  loading,
  error,
  playlist,
  saving,
  savedUrl,
  onSave,
  onClose,
  onPickTrack,
}: {
  loading: boolean;
  error: string | null;
  playlist: AutoPlaylistFull | undefined;
  saving: boolean;
  savedUrl: string | null;
  onSave: () => void;
  onClose: () => void;
  onPickTrack: (id: string) => void;
}) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-6 sm:p-8 flex flex-col gap-5 flex-1 min-h-0">
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <button
          type="button"
          onClick={onClose}
          className="flex items-center gap-2 rounded-full border border-zinc-800 bg-zinc-900/60 hover:bg-zinc-900 text-zinc-300 px-3 py-1.5 text-xs"
        >
          <span aria-hidden>←</span> Back
        </button>
        <div className="flex items-center gap-2">
          {savedUrl ? (
            <a
              href={savedUrl}
              target="_blank"
              rel="noopener"
              className="rounded-full bg-emerald-500 px-3 py-1.5 text-xs font-semibold text-black hover:bg-emerald-400"
            >
              Open in Spotify ↗
            </a>
          ) : (
            <button
              onClick={onSave}
              disabled={saving || !playlist}
              className="rounded-full border border-emerald-500 text-emerald-400 px-3 py-1.5 text-xs font-semibold hover:bg-emerald-500/10 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {saving ? "Saving…" : "Save to Spotify"}
            </button>
          )}
        </div>
      </div>

      {loading && !playlist && (
        <p className="text-zinc-500">Loading playlist…</p>
      )}
      {error && !loading && <p className="text-red-400">{error}</p>}

      {playlist && (
        <>
          <header className="flex flex-col gap-1">
            <span className="text-xs uppercase tracking-[0.2em] text-emerald-400">
              Smart playlist
            </span>
            <h2 className="text-2xl sm:text-3xl font-semibold text-white capitalize">
              {playlist.name}
            </h2>
            <p className="text-zinc-500 text-sm">
              {playlist.track_count.toLocaleString()} tracks
              {playlist.created_at &&
                ` · created ${new Date(playlist.created_at).toLocaleDateString()}`}
            </p>
            {playlist.description && (
              <p className="text-zinc-400 text-sm mt-1">{playlist.description}</p>
            )}
          </header>

          <div className="flex-1 min-h-0 overflow-y-auto -mx-2 px-2">
            <ul className="divide-y divide-zinc-900">
              {playlist.tracks.map((t) => (
                <li key={t.id}>
                  <button
                    type="button"
                    onClick={() => onPickTrack(t.id)}
                    className="flex items-center gap-4 py-3 text-left w-full -mx-3 px-3 rounded-md hover:bg-zinc-900/50 transition-colors"
                  >
                    {t.image_url && (
                      /* eslint-disable-next-line @next/next/no-img-element */
                      <img
                        src={t.image_url}
                        alt=""
                        className="h-12 w-12 rounded object-cover flex-shrink-0"
                      />
                    )}
                    <div className="flex-1 min-w-0">
                      <div className="text-white truncate">{t.name}</div>
                      <div className="text-sm text-zinc-400 truncate">
                        {t.artists.join(", ")} · {t.album}
                      </div>
                    </div>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        </>
      )}
    </section>
  );
}
