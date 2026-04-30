"use client";

import type { AutoSummary } from "../types";
import { AutoCard } from "./AutoCard";

export function SmartPlaylists({
  autos,
  autosLoading,
  regenerating,
  autoError,
  autoSaving,
  autoSaved,
  isSyncing,
  isSyncDone,
  onRegenerate,
  onOpen,
  onSave,
  onPickTrack,
}: {
  autos: AutoSummary[] | null;
  autosLoading: boolean;
  regenerating: boolean;
  autoError: string | null;
  autoSaving: string | null;
  autoSaved: Record<string, string>;
  isSyncing: boolean;
  isSyncDone: boolean;
  onRegenerate: () => void;
  onOpen: (id: string) => void;
  onSave: (p: AutoSummary) => void;
  onPickTrack: (id: string) => void;
}) {
  return (
    <aside className="flex flex-col gap-5 min-h-0 lg:max-h-[calc(100vh-3rem)]">
      <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-5 flex flex-col gap-4 flex-1 min-h-0">
        <div className="flex items-start justify-between gap-2 flex-wrap">
          <div>
            <h2 className="text-base font-semibold text-white">Smart playlists</h2>
            <p className="text-zinc-500 text-xs mt-0.5">
              Auto-clustered by metadata.
            </p>
          </div>
          <button
            onClick={onRegenerate}
            disabled={regenerating || isSyncing || !isSyncDone}
            className="rounded-full border border-emerald-500 text-emerald-400 px-3 py-1 text-xs font-semibold hover:bg-emerald-500/10 disabled:opacity-50 disabled:cursor-not-allowed flex-shrink-0"
          >
            {regenerating
              ? "…"
              : autos && autos.length > 0
                ? "Regenerate"
                : "Generate"}
          </button>
        </div>
        {autoError && <p className="text-red-400 text-xs">{autoError}</p>}
        {autosLoading && !autos ? (
          <p className="text-zinc-500 text-sm">Loading…</p>
        ) : !autos || autos.length === 0 ? (
          <p className="text-zinc-500 text-xs">No smart playlists yet.</p>
        ) : (
          <div className="flex-1 min-h-0 overflow-y-auto -mx-1 px-1 flex flex-col gap-3">
            {autos.map((p) => (
              <AutoCard
                key={p.id}
                p={p}
                saving={autoSaving === p.id}
                savedUrl={autoSaved[p.id] ?? null}
                onOpen={() => onOpen(p.id)}
                onSave={() => onSave(p)}
                onPickTrack={onPickTrack}
              />
            ))}
          </div>
        )}
      </section>
    </aside>
  );
}
