"use client";

import type { AutoSummary } from "../types";

export function AutoCard({
  p,
  saving,
  savedUrl,
  onOpen,
  onSave,
  onPickTrack,
}: {
  p: AutoSummary;
  saving: boolean;
  savedUrl: string | null;
  onOpen: () => void;
  onSave: () => void;
  onPickTrack: (id: string) => void;
}) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-black/40 p-5 flex flex-col gap-3">
      <div className="flex items-start justify-between gap-2">
        <button
          type="button"
          onClick={onOpen}
          className="flex-1 min-w-0 text-left rounded -m-1 p-1 hover:bg-zinc-900/60"
        >
          <div className="text-white font-semibold capitalize truncate">
            {p.name}
          </div>
          <div className="text-xs text-zinc-500">{p.track_count} tracks</div>
        </button>
        {savedUrl ? (
          <a
            href={savedUrl}
            target="_blank"
            rel="noopener"
            className="rounded-full bg-emerald-500 px-3 py-1 text-xs font-semibold text-black hover:bg-emerald-400 flex-shrink-0"
          >
            Open ↗
          </a>
        ) : (
          <button
            onClick={onSave}
            disabled={saving}
            className="rounded-full border border-emerald-500 text-emerald-400 px-3 py-1 text-xs font-semibold hover:bg-emerald-500/10 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex-shrink-0"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        )}
      </div>
      <div className="flex flex-col gap-1">
        {p.sample.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => onPickTrack(t.id)}
            className="flex items-center gap-2 min-w-0 text-left rounded px-1 py-0.5 -mx-1 hover:bg-zinc-900/50"
          >
            {t.image_url && (
              /* eslint-disable-next-line @next/next/no-img-element */
              <img
                src={t.image_url}
                alt=""
                className="h-6 w-6 rounded object-cover flex-shrink-0"
              />
            )}
            <span className="text-zinc-300 text-xs truncate">
              {t.name}{" "}
              <span className="text-zinc-500">— {t.artists.join(", ")}</span>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
