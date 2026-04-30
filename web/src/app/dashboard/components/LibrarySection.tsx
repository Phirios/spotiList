"use client";

import type { LibraryResponse, Row } from "../types";
import { TrackList } from "./TrackList";

export function LibrarySection({
  library,
  libraryQ,
  setLibraryQ,
  libraryLoading,
  libraryMoreLoading,
  onScroll,
  libraryRows,
  selectedTrack,
  onSelectTrack,
}: {
  library: LibraryResponse | null;
  libraryQ: string;
  setLibraryQ: (q: string) => void;
  libraryLoading: boolean;
  libraryMoreLoading: boolean;
  onScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  libraryRows: Row[];
  selectedTrack: string | null;
  onSelectTrack: (id: string) => void;
}) {
  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-6 sm:p-8 flex flex-col gap-5 flex-1 min-h-0">
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-1 rounded-full bg-zinc-900 p-1 self-start">
          <button
            type="button"
            className="rounded-full bg-zinc-800 px-3 py-1 text-xs text-white font-medium"
          >
            Liked
          </button>
          <button
            type="button"
            disabled
            title="Coming soon"
            className="rounded-full px-3 py-1 text-xs text-zinc-600 cursor-not-allowed"
          >
            Custom · soon
          </button>
        </div>
        <div className="flex items-start justify-between gap-3 flex-wrap">
          <div>
            <h2 className="text-xl font-semibold text-white mb-1">
              Your library
            </h2>
            <p className="text-zinc-400 text-sm">
              {library
                ? `${library.total.toLocaleString()} ${library.q ? "matches" : "liked songs"} · showing ${library.items.length}`
                : "Loading…"}
            </p>
          </div>
          <input
            type="text"
            value={libraryQ}
            onChange={(e) => setLibraryQ(e.target.value)}
            placeholder="Search by name, artist, album…"
            className="rounded-full bg-black border border-zinc-800 px-4 py-2 text-sm text-white placeholder:text-zinc-600 focus:outline-none focus:border-emerald-500 w-full sm:w-72"
          />
        </div>
        {libraryLoading && (
          <p className="text-zinc-500 text-xs">Searching…</p>
        )}
        {library && library.items.length === 0 && !libraryLoading && (
          <p className="text-zinc-500 text-sm">No matches.</p>
        )}
      </div>
      <div
        onScroll={onScroll}
        className="flex-1 min-h-0 overflow-y-auto -mx-2 px-2"
      >
        <TrackList
          rows={libraryRows}
          selected={selectedTrack}
          onSelect={onSelectTrack}
        />
        {library && library.items.length < library.total && (
          <div className="py-4 text-center text-xs text-zinc-500">
            {libraryMoreLoading ? "Loading more…" : "Scroll for more"}
          </div>
        )}
        {library &&
          library.items.length >= library.total &&
          library.total > 30 && (
            <div className="py-4 text-center text-xs text-zinc-600">
              End of {library.total.toLocaleString()} results
            </div>
          )}
      </div>
    </section>
  );
}
