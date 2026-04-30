"use client";

import { useEffect } from "react";
import type { SimilarTrack, TrackInfo } from "../types";
import { Field } from "./Field";

export function TrackPanel({
  loading,
  error,
  info,
  similar,
  similarLoading,
  onClose,
  onPick,
}: {
  loading: boolean;
  error: string | null;
  info: TrackInfo | undefined;
  similar: SimilarTrack[] | undefined;
  similarLoading: boolean;
  onClose: () => void;
  onPick: (id: string) => void;
}) {
  // Esc key closes the panel.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-6 sm:p-8 flex flex-col gap-5 flex-1 min-h-0">
      <div className="flex items-center justify-between gap-3">
        <button
          type="button"
          onClick={onClose}
          className="flex items-center gap-2 rounded-full border border-zinc-800 bg-zinc-900/60 hover:bg-zinc-900 text-zinc-300 px-3 py-1.5 text-xs"
        >
          <span aria-hidden>←</span> Back to library
        </button>
        {info?.spotify_url && (
          <a
            href={info.spotify_url}
            target="_blank"
            rel="noopener"
            className="rounded-full bg-emerald-500 px-3 py-1.5 text-xs font-semibold text-black hover:bg-emerald-400"
          >
            Open in Spotify ↗
          </a>
        )}
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto -mx-2 px-2">
        {loading && !info && (
          <p className="text-zinc-500">Loading metadata…</p>
        )}
        {error && !loading && <p className="text-red-400">{error}</p>}
        {info && (
          <div className="flex flex-col gap-6">
            <header className="flex items-start gap-4">
              {info.album.image_url && (
                /* eslint-disable-next-line @next/next/no-img-element */
                <img
                  src={info.album.image_url}
                  alt=""
                  className="h-28 w-28 sm:h-32 sm:w-32 rounded-lg object-cover flex-shrink-0"
                />
              )}
              <div className="min-w-0 flex flex-col">
                <h2 className="text-2xl sm:text-3xl font-semibold text-white truncate">
                  {info.name}
                </h2>
                <div className="text-zinc-300 text-sm sm:text-base truncate mt-1">
                  {info.artists.map((a) => a.name).join(", ")}
                </div>
                <div className="text-zinc-500 text-sm truncate">
                  {info.album.name}
                  {info.album.release_date && (
                    <span> · {info.album.release_date.slice(0, 4)}</span>
                  )}
                </div>
              </div>
            </header>

            <div className="grid grid-cols-2 sm:grid-cols-3 gap-4">
              <Field label="BPM">
                {info.bpm ? (
                  <span className="text-white tabular-nums">
                    {info.bpm.tempo.toFixed(1)}
                  </span>
                ) : (
                  <span className="text-zinc-600">—</span>
                )}
              </Field>
              <Field label="Popularity">
                {info.popularity !== null ? (
                  <span className="text-white tabular-nums">
                    {info.popularity}
                  </span>
                ) : (
                  <span className="text-zinc-600">—</span>
                )}
              </Field>
              <Field label="Duration">
                <span className="text-white tabular-nums">
                  {formatDuration(info.duration_ms)}
                </span>
              </Field>
            </div>

            <div>
              <div className="text-xs uppercase tracking-wider text-zinc-500 mb-2">
                Genres
              </div>
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
                <span className="text-zinc-600 text-sm">—</span>
              )}
            </div>

            <div className="grid gap-6 lg:grid-cols-[1fr_280px]">
              <div>
                <div className="text-xs uppercase tracking-wider text-zinc-500 mb-2">
                  Lyrics
                </div>
                {info.lyrics?.synced ? (
                  <pre className="whitespace-pre-wrap font-sans text-zinc-300 text-sm leading-relaxed">
                    {info.lyrics.synced}
                  </pre>
                ) : info.lyrics?.plain ? (
                  <pre className="whitespace-pre-wrap font-sans text-zinc-300 text-sm leading-relaxed">
                    {info.lyrics.plain}
                  </pre>
                ) : info.lyrics?.instrumental ? (
                  <p className="text-zinc-500 text-sm">Instrumental</p>
                ) : (
                  <p className="text-zinc-600 text-sm">—</p>
                )}
              </div>

              <div>
                <div className="text-xs uppercase tracking-wider text-zinc-500 mb-2">
                  Similar in your library
                </div>
                {similarLoading && !similar ? (
                  <p className="text-zinc-500 text-sm">Looking…</p>
                ) : similar && similar.length > 0 ? (
                  <ul className="flex flex-col gap-1">
                    {similar.map((s) => (
                      <li key={s.id}>
                        <button
                          type="button"
                          onClick={() => onPick(s.id)}
                          className="flex items-center gap-3 w-full py-1.5 px-2 -mx-2 rounded hover:bg-zinc-900/50 text-left"
                        >
                          {s.image_url && (
                            /* eslint-disable-next-line @next/next/no-img-element */
                            <img
                              src={s.image_url}
                              alt=""
                              className="h-8 w-8 rounded object-cover flex-shrink-0"
                            />
                          )}
                          <span className="flex-1 min-w-0 text-sm">
                            <span className="text-white truncate block">
                              {s.name}
                            </span>
                            <span className="text-zinc-500 text-xs truncate block">
                              {s.artists.join(", ")}
                            </span>
                          </span>
                          <span className="text-xs text-zinc-500 tabular-nums flex-shrink-0">
                            {s.score.toFixed(2)}
                          </span>
                        </button>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-zinc-600 text-sm">—</p>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function formatDuration(ms: number): string {
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
