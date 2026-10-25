// The one place the Verse turns free text into a choice. Every route (an
// agent command, a chat intent, a planet setting) is a typed entry with
// example phrasings; text is embedded as a sparse vector of hashed word,
// word-pair, and character-trigram features, and the route whose closest
// example has the highest cosine similarity wins if it clears a threshold.
// Nothing downstream matches keywords: callers get a route id and act on it.
'use strict';

const Semantic = (() => {
  const DIM = 4096;
  const hash = (s) => {
    let h = 2166136261;
    for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
    return (h >>> 0) % DIM;
  };
  const words = (text) => text.toLowerCase().normalize('NFKD').replace(/[^a-z0-9 ]+/g, ' ').split(/\s+/).filter(Boolean);
  // Words too common to say anything about which route was meant.
  const FILLER = new Set(['the', 'a', 'an', 'please', 'can', 'you', 'could', 'would', 'to', 'of', 'my', 'me', 'i', 'it', 'is', 'and', 'some', 'this', 'that', 'want', 'like', 'make', 'be']);

  function embed(text) {
    const v = new Map();
    const add = (f, w) => { const k = hash(f); v.set(k, (v.get(k) || 0) + w); };
    const ws = words(text);
    const content = ws.filter((w) => !FILLER.has(w));
    for (const w of content) {
      add('w:' + w, 1);
      const p = '<' + w + '>';
      for (let i = 0; i + 3 <= p.length; i++) add('c:' + p.slice(i, i + 3), 0.45);
    }
    for (let i = 0; i + 1 < content.length; i++) add('b:' + content[i] + ' ' + content[i + 1], 0.7);
    let n = 0;
    for (const x of v.values()) n += x * x;
    n = Math.sqrt(n) || 1;
    for (const [k, x] of v) v.set(k, x / n);
    return v;
  }

  function cosine(a, b) {
    if (a.size > b.size) [a, b] = [b, a];
    let s = 0;
    for (const [k, x] of a) { const y = b.get(k); if (y) s += x * y; }
    return s;
  }

  // routes: [{ id, examples: [...], ...anything the caller wants back }]
  function selector(routes, { threshold = 0.32 } = {}) {
    const table = routes.map((r) => ({ route: r, vecs: r.examples.map(embed) }));
    function rank(text) {
      const q = embed(text);
      return table
        .map(({ route, vecs }) => ({ route, score: Math.max(...vecs.map((v) => cosine(q, v))) }))
        .sort((a, b) => b.score - a.score);
    }
    return {
      rank,
      // The best route, or null when nothing is close enough.
      select(text) {
        const [best, next] = rank(text);
        if (!best || best.score < threshold) return null;
        return { ...best.route, score: best.score, margin: best.score - (next ? next.score : 0) };
      },
    };
  }

  return { embed, cosine, selector };
})();
