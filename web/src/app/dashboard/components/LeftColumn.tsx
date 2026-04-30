"use client";

import type {
  GeneratedPlaylist,
  Me,
  SyncStatus,
} from "../types";
import { ProfileMenu } from "./ProfileMenu";
import { SyncCard } from "./SyncCard";

export function LeftColumn({
  me,
  error,
  sync,
  onResync,
  vibe,
  setVibe,
  generating,
  generated,
  generateError,
  saving,
  saved,
  saveError,
  isSyncing,
  isSyncDone,
  selectedTrack,
  onGenerate,
  onSave,
  onSelectTrack,
  onLogout,
}: {
  me: Me | null;
  error: string | null;
  sync: SyncStatus | null;
  onResync: () => void;
  vibe: string;
  setVibe: (v: string) => void;
  generating: boolean;
  generated: GeneratedPlaylist | null;
  generateError: string | null;
  saving: boolean;
  saved: { url: string; name: string } | null;
  saveError: string | null;
  isSyncing: boolean;
  isSyncDone: boolean;
  selectedTrack: string | null;
  onGenerate: (e: React.FormEvent) => void;
  onSave: () => void;
  onSelectTrack: (id: string) => void;
  onLogout: () => void;
}) {
  return (
    <aside className="flex flex-col gap-5 min-h-0 lg:max-h-[calc(100vh-3rem)] lg:overflow-y-auto pr-1">
      <header className="flex items-center gap-3">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src="/logo.png" alt="spoti" className="h-10 w-10 rounded-lg" />
        <div className="flex flex-col">
          <span className="text-xs uppercase tracking-[0.25em] text-emerald-400">
            spoti
          </span>
          <span className="text-xs text-zinc-500">vibe playlists</span>
        </div>
      </header>

      {error && <p className="text-red-400 text-xs">{error}</p>}

      <SyncCard sync={sync} onResync={onResync} />

      <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-5 flex flex-col gap-4">
        <div>
          <h2 className="text-base font-semibold text-white">
            Generate a vibe playlist
          </h2>
          <p className="text-zinc-500 text-xs mt-0.5">Describe a mood.</p>
        </div>
        <form onSubmit={onGenerate} className="flex flex-col gap-2">
          <input
            type="text"
            value={vibe}
            onChange={(e) => setVibe(e.target.value)}
            placeholder="late-night drive…"
            className="rounded-full bg-black border border-zinc-800 px-4 py-2 text-sm text-white placeholder:text-zinc-600 focus:outline-none focus:border-emerald-500"
          />
          <button
            type="submit"
            disabled={generating || !vibe.trim() || isSyncing || !isSyncDone}
            className="rounded-full bg-emerald-500 px-4 py-2 text-sm font-semibold text-black hover:bg-emerald-400 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {generating ? "Generating…" : "Generate"}
          </button>
        </form>
        {generateError && (
          <p className="text-red-400 text-xs">{generateError}</p>
        )}
        {generated && (
          <div className="flex flex-col gap-2 -mx-1">
            <div className="px-1 flex items-center justify-between gap-2 text-xs">
              <span className="text-zinc-500">
                {generated.items.length} of {generated.considered}
              </span>
              {saved ? (
                <a
                  href={saved.url}
                  target="_blank"
                  rel="noopener"
                  className="rounded-full bg-emerald-500 px-3 py-1 text-xs font-semibold text-black hover:bg-emerald-400"
                >
                  Open ↗
                </a>
              ) : (
                <button
                  type="button"
                  onClick={onSave}
                  disabled={saving || generated.items.length === 0}
                  className="rounded-full border border-emerald-500 text-emerald-400 px-3 py-1 text-xs font-semibold hover:bg-emerald-500/10 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {saving ? "Saving…" : "Save"}
                </button>
              )}
            </div>
            {saveError && <p className="text-red-400 text-xs px-1">{saveError}</p>}
            <ul className="max-h-[28rem] overflow-y-auto divide-y divide-zinc-900">
              {generated.items.map((t) => (
                <li key={t.id}>
                  <button
                    type="button"
                    onClick={() => onSelectTrack(t.id)}
                    className={`flex items-center gap-2 py-2 px-1 w-full text-left rounded transition-colors ${
                      selectedTrack === t.id
                        ? "bg-emerald-500/10"
                        : "hover:bg-zinc-900/50"
                    }`}
                  >
                    {t.image_url && (
                      /* eslint-disable-next-line @next/next/no-img-element */
                      <img
                        src={t.image_url}
                        alt=""
                        className="h-8 w-8 rounded object-cover flex-shrink-0"
                      />
                    )}
                    <div className="flex-1 min-w-0">
                      <div className="text-white text-sm truncate">{t.name}</div>
                      <div className="text-xs text-zinc-500 truncate">
                        {t.artists.join(", ")}
                      </div>
                    </div>
                    <span className="text-xs text-zinc-600 tabular-nums">
                      {t.score.toFixed(2)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>

      <div className="flex-1" />

      {me && <ProfileMenu me={me} onLogout={onLogout} />}
    </aside>
  );
}
