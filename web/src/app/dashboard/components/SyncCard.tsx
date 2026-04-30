"use client";

import { useEffect, useState } from "react";
import { STAGE_LABEL, type SyncStatus } from "../types";

export function SyncCard({
  sync,
  onResync,
}: {
  sync: SyncStatus | null;
  onResync: () => void;
}) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (sync?.status !== "running") return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [sync?.status]);

  if (!sync) return null;

  if (sync.status === "done") {
    return (
      <section className="rounded-2xl border border-emerald-900/50 bg-emerald-950/20 px-6 py-4 flex items-center justify-between gap-3">
        <div className="text-sm text-emerald-300">
          Library synced{" "}
          {sync.finished_at && (
            <span className="text-emerald-300/60">
              · {new Date(sync.finished_at).toLocaleString()}
            </span>
          )}
        </div>
        <button
          onClick={onResync}
          className="rounded-full border border-emerald-700 text-emerald-300 px-3 py-1 text-xs hover:bg-emerald-900/30"
        >
          Resync
        </button>
      </section>
    );
  }

  if (sync.status === "failed") {
    return (
      <section className="rounded-2xl border border-red-900/50 bg-red-950/20 px-6 py-4 flex items-center justify-between gap-3">
        <div className="text-sm text-red-300">
          Sync failed{sync.error ? `: ${sync.error}` : ""}
        </div>
        <button
          onClick={onResync}
          className="rounded-full border border-red-700 text-red-300 px-3 py-1 text-xs hover:bg-red-900/30"
        >
          Retry
        </button>
      </section>
    );
  }

  const stage = sync.stage ?? "starting";
  const label = STAGE_LABEL[stage] ?? "Working";
  const total = sync.total || 0;
  const progress = sync.progress || 0;
  const pct =
    total > 0 ? Math.min(100, Math.round((progress / total) * 100)) : null;
  let etaSeconds: number | null = null;
  if (sync.started_at && progress > 0 && total > progress) {
    const startedMs = new Date(sync.started_at).getTime();
    const elapsed = (now - startedMs) / 1000;
    const rate = progress / elapsed;
    if (rate > 0) {
      etaSeconds = Math.max(1, Math.round((total - progress) / rate));
    }
  }

  return (
    <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-6 flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-sm uppercase tracking-[0.2em] text-emerald-400">
            Syncing your library
          </div>
          <div className="text-white text-lg mt-1">{label}</div>
        </div>
        {etaSeconds !== null && (
          <div className="text-right">
            <div className="text-xs uppercase tracking-wider text-zinc-500">
              ETA
            </div>
            <div className="text-white tabular-nums">
              {formatEta(etaSeconds)}
            </div>
          </div>
        )}
      </div>
      <div className="h-2 rounded-full bg-zinc-800 overflow-hidden">
        <div
          className="h-full bg-emerald-500 transition-[width] duration-500"
          style={{ width: `${pct ?? 5}%` }}
        />
      </div>
      <div className="flex items-center justify-between text-xs text-zinc-500 tabular-nums">
        <span>
          {progress.toLocaleString()}
          {total > 0 ? ` / ${total.toLocaleString()}` : ""}
        </span>
        {pct !== null && <span>{pct}%</span>}
      </div>
    </section>
  );
}

function formatEta(sec: number): string {
  if (sec < 60) return `${sec}s`;
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return s ? `${m}m ${s}s` : `${m}m`;
}
