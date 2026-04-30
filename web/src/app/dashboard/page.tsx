"use client";

import { useEffect, useState } from "react";

type Me = {
  id: string;
  spotify_id: string;
  display_name: string | null;
  email: string | null;
};

export default function Dashboard() {
  const [me, setMe] = useState<Me | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch("/api/auth/me", { credentials: "include" })
      .then(async (r) => {
        if (r.status === 401 || r.status === 403) {
          window.location.href = "/api/auth/login";
          return;
        }
        if (!r.ok) throw new Error(`status ${r.status}`);
        const data = (await r.json()) as Me;
        setMe(data);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function logout() {
    await fetch("/api/auth/logout", { method: "POST", credentials: "include" });
    window.location.href = "/";
  }

  return (
    <div className="flex flex-1 flex-col bg-black font-sans text-zinc-200">
      <main className="flex flex-1 flex-col gap-12 px-6 py-20 sm:px-12 lg:px-20 max-w-5xl mx-auto w-full">
        <header className="flex flex-col gap-3">
          <span className="text-sm uppercase tracking-[0.3em] text-emerald-400">
            dashboard
          </span>
          <h1 className="text-3xl sm:text-5xl font-semibold tracking-tight text-white">
            {loading
              ? "Loading…"
              : me
                ? `Hey ${me.display_name ?? me.spotify_id}`
                : "Not logged in"}
          </h1>
          {error && <p className="text-red-400 text-sm">{error}</p>}
        </header>

        {me && (
          <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-8 sm:p-10 flex flex-col gap-6">
            <div>
              <h2 className="text-xl font-semibold text-white mb-2">
                Generate a vibe playlist
              </h2>
              <p className="text-zinc-400">
                Coming soon. The bot is being wired up — for now you&rsquo;re
                successfully connected to Spotify.
              </p>
            </div>
            <div className="flex gap-3">
              <button
                onClick={logout}
                className="rounded-full border border-zinc-700 px-5 py-2 text-sm hover:bg-zinc-900"
              >
                Log out
              </button>
            </div>
          </section>
        )}
      </main>
    </div>
  );
}
