export default function Home() {
  return (
    <div className="flex flex-1 flex-col bg-black font-sans text-zinc-200">
      <main className="flex flex-1 flex-col gap-20 px-6 py-20 sm:px-12 lg:px-20 max-w-5xl mx-auto w-full">
        <header className="flex flex-col gap-6">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src="/logo.png"
            alt="spoti"
            className="h-14 w-14 rounded-xl"
          />
          <span className="text-sm uppercase tracking-[0.3em] text-emerald-400">
            spoti.phirios.com
          </span>
          <h1 className="text-4xl sm:text-6xl font-semibold tracking-tight text-white max-w-3xl">
            Vibe-based playlists from your liked songs.
          </h1>
          <p className="text-lg text-zinc-400 max-w-2xl leading-relaxed">
            An NLP project that reads your Spotify library and turns a plain-language
            mood — &ldquo;late-night drive after a long week&rdquo;, &ldquo;deep focus,
            no vocals&rdquo;, &ldquo;Sunday morning slow&rdquo; — into a playlist that
            actually matches.
          </p>
          <div>
            <a
              href="/api/auth/login"
              className="inline-flex items-center gap-2 rounded-full bg-emerald-500 px-6 py-3 text-sm font-semibold text-black hover:bg-emerald-400 transition-colors"
            >
              Log in with Spotify
            </a>
          </div>
        </header>

        <section className="grid gap-8 sm:grid-cols-2">
          <Card title="What it does">
            Pulls your liked songs from Spotify, enriches each track with audio features
            (BPM, energy, valence) and genre tags, then matches a free-form vibe prompt
            to the right subset of your library.
          </Card>
          <Card title="How it works">
            Track metadata via Spotify, tempo via GetSongBPM, lyrics via LRCLIB, genre
            tags via Last.fm. A language model embeds the user prompt and the enriched
            track features into the same space and ranks by similarity.
          </Card>
          <Card title="Why">
            Coursework for an NLP class — exploring how natural-language descriptions
            of mood map onto structured audio features and crowd-sourced tags.
          </Card>
          <Card title="Stack">
            Rust (axum) backend for the metadata service, Next.js + Tailwind for the
            web UI, Python for the NLP / embedding layer. Deployed on a personal
            Kubernetes cluster.
          </Card>
        </section>

        <section className="rounded-2xl border border-zinc-800 bg-zinc-950 p-8 sm:p-10">
          <h2 className="text-xl font-semibold text-white mb-3">Status</h2>
          <p className="text-zinc-400 leading-relaxed">
            In development. The backend track-info service is live; the bot and web UI
            are next. Source will be published once the homework is graded.
          </p>
        </section>
      </main>

      <footer className="border-t border-zinc-900 px-6 sm:px-12 lg:px-20 py-10 text-sm text-zinc-500">
        <div className="max-w-5xl mx-auto w-full flex flex-col sm:flex-row gap-4 sm:items-center sm:justify-between">
          <div>
            &copy; {new Date().getFullYear()} phirios. NLP coursework project.
          </div>
          <div className="flex flex-wrap gap-x-6 gap-y-2">
            <span>
              BPM data:{" "}
              <a
                href="https://getsongbpm.com"
                className="text-emerald-400 hover:underline"
                target="_blank"
                rel="noopener"
              >
                GetSongBPM
              </a>
            </span>
            <span>
              Lyrics:{" "}
              <a
                href="https://lrclib.net"
                className="text-emerald-400 hover:underline"
                target="_blank"
                rel="noopener"
              >
                LRCLIB
              </a>
            </span>
            <span>
              Genres:{" "}
              <a
                href="https://www.last.fm/api"
                className="text-emerald-400 hover:underline"
                target="_blank"
                rel="noopener"
              >
                Last.fm
              </a>
            </span>
          </div>
        </div>
      </footer>
    </div>
  );
}

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-2xl border border-zinc-800 bg-zinc-950 p-6 sm:p-8">
      <h3 className="text-lg font-semibold text-white mb-2">{title}</h3>
      <p className="text-zinc-400 leading-relaxed">{children}</p>
    </div>
  );
}
