"use client";

import type { Row } from "../types";

export function TrackList({
  rows,
  selected,
  onSelect,
}: {
  rows: Row[];
  selected: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <ul className="divide-y divide-zinc-900">
      {rows.map((row) => (
        <li key={row.id}>
          <button
            type="button"
            onClick={() => onSelect(row.id)}
            className={`flex items-center gap-4 py-3 text-left w-full -mx-3 px-3 rounded-md transition-colors ${
              selected === row.id
                ? "bg-emerald-500/10"
                : "hover:bg-zinc-900/50"
            }`}
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
        </li>
      ))}
    </ul>
  );
}
