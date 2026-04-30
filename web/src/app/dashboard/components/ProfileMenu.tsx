"use client";

import { useState } from "react";
import type { Me } from "../types";

export function ProfileMenu({
  me,
  onLogout,
}: {
  me: Me;
  onLogout: () => void;
}) {
  const [open, setOpen] = useState(false);
  const initial = (me.display_name ?? me.spotify_id).charAt(0).toUpperCase();

  return (
    <div className="relative">
      {open && (
        <div className="absolute bottom-full left-0 right-0 mb-2 rounded-xl border border-zinc-800 bg-zinc-950 p-3 shadow-xl flex flex-col gap-2 z-10">
          <div className="text-xs text-zinc-500">Logged in as</div>
          <div className="text-white text-sm truncate">
            {me.display_name ?? me.spotify_id}
          </div>
          {me.email && (
            <div className="text-zinc-500 text-xs truncate">{me.email}</div>
          )}
          <button
            onClick={onLogout}
            className="mt-1 rounded-full border border-zinc-700 px-3 py-1.5 text-xs hover:bg-zinc-900 text-left"
          >
            Log out
          </button>
        </div>
      )}
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-3 w-full rounded-xl border border-zinc-800 bg-zinc-950 px-3 py-2 hover:bg-zinc-900 text-left"
      >
        <span className="h-8 w-8 rounded-full bg-emerald-500 text-black flex items-center justify-center font-semibold text-sm">
          {initial}
        </span>
        <span className="flex-1 min-w-0">
          <span className="block text-white text-sm truncate">
            {me.display_name ?? me.spotify_id}
          </span>
          <span className="block text-zinc-500 text-xs">
            {open ? "Close" : "Account"}
          </span>
        </span>
        <span className="text-zinc-500 text-xs">{open ? "▾" : "▴"}</span>
      </button>
    </div>
  );
}
