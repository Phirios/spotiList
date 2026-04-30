# Design notes

## Retrieval: chose B2 (dense embeddings, local model)

Going with a Python sentence-transformers sidecar service. Backend embeds tracks
once, caches in Postgres, and ranks against an embedded vibe prompt with cosine
similarity.

## Future option: C — hybrid retrieval

If pure dense embeddings turn out to miss too many relevant tracks (or grade
better with more variety in techniques), upgrade to a hybrid retriever that
combines three signals:

1. **Dense embeddings** (already in B2) — captures semantic similarity, e.g.
   `"melancholy"` ≈ `"blue"`.
2. **BM25 / sparse retrieval** over the same text fields (artist + title +
   genres + key lyric lines). Catches literal keyword matches the embedding
   model misses (rare genre names, song titles, artist references).
3. **Structured filters extracted from the prompt** — hard constraints applied
   before / alongside ranking. Examples:
   - `"slow"` → BPM ≤ 90
   - `"upbeat run playlist"` → BPM ≥ 130
   - `"no vocals"` → instrumental flag true
   - `"sad"` → valence < 0.4 *(only if we ever recover audio features)*

   Filter extraction can be done with a small fine-tuned model, regex over a
   keyword vocabulary, or a single LLM prompt that returns JSON.

Score fusion options: weighted sum (e.g. `0.6·embedding + 0.3·BM25 +
0.1·filter_match`) or **Reciprocal Rank Fusion** (RRF), which is simpler to
tune because it only depends on rank, not absolute scores.

### What changes vs B2

- Add a tokenizer + BM25 index per user (compute once, persist alongside
  embeddings in Postgres or as a Tantivy index volume).
- Add a `prompt → filters` extraction step (could share the embedder pod's
  Python runtime).
- Add a fusion step in the Rust backend that merges the three ranked lists.

### Why we're not doing it now

- B2 alone usually returns acceptable results for a coursework demo.
- BM25 + filter extraction roughly doubles the surface area to test, debug, and
  explain in the writeup.
- The `valence` / `energy` filters are nullified by the Spotify Audio Features
  deprecation for new apps, so the structured-filter signal is weaker than it
  would be with full audio features.

If we *do* upgrade to C, the existing embedding cache stays useful — we just
add the other two signals on top.
