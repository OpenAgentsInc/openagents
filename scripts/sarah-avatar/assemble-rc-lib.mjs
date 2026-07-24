// Pure helpers for Sarah episode RC assemble (#9234).
// One A/B/C picture cut with continuous Sarah audio, cut-point Finder QC,
// silence-derived T1, optional mid cutaway, and transcript lock checks.

import { looksLikeDesktopOrFinderTail } from "../omega-screen-control/recording-lifecycle.mjs";

export { looksLikeDesktopOrFinderTail };

/** Default T2 fraction of Sarah-master duration when --t2 is omitted. */
export const DEFAULT_T2_FRACTION = 0.78;

/** Default silence search window for post-spoken pause (Episode 262: after "zed"). */
export const DEFAULT_T1_SEARCH = Object.freeze({
  fromSec: 14,
  toSec: 20,
  noiseDb: -35,
  minSilenceSec: 0.25,
  padAfterSilenceSec: 0.2,
});

/**
 * QC sample offsets relative to T2 (seconds), all inside Part B mid picture.
 * Do not sample at exact T2: that frame is the Sarah return and would false-
 * positive against a dark product reference. Episode 262 leaked Finder in the
 * last ~0.9s of the mid — sample the mid tail just before the cut.
 */
export const DEFAULT_QC_OFFSETS_SEC = Object.freeze([-1, -0.5, -1 / 24]);

/**
 * Parse ffmpeg silencedetect stderr lines into { start, end, duration }[].
 */
export function parseSilenceDetectLog(text) {
  const lines = String(text || "").split(/\r?\n/);
  const starts = [];
  const ends = [];
  for (const line of lines) {
    const startMatch = line.match(/silence_start:\s*([0-9.]+)/i);
    if (startMatch) {
      starts.push(Number(startMatch[1]));
      continue;
    }
    const endMatch = line.match(/silence_end:\s*([0-9.]+)\s*\|\s*silence_duration:\s*([0-9.]+)/i);
    if (endMatch) {
      ends.push({
        end: Number(endMatch[1]),
        duration: Number(endMatch[2]),
      });
    }
  }
  const intervals = [];
  for (const endInfo of ends) {
    const start = endInfo.end - endInfo.duration;
    if ([start, endInfo.end, endInfo.duration].every((n) => Number.isFinite(n))) {
      intervals.push({
        start,
        end: endInfo.end,
        duration: endInfo.duration,
      });
    }
  }
  // Orphan silence_start without end (file ends in silence).
  if (starts.length > ends.length) {
    const orphan = starts[starts.length - 1];
    if (Number.isFinite(orphan)) {
      intervals.push({ start: orphan, end: orphan, duration: 0 });
    }
  }
  return intervals.sort((a, b) => a.start - b.start);
}

/**
 * Choose T1 from silence intervals inside a search window.
 * Prefer the longest silence that ends inside [fromSec, toSec], then place
 * T1 at silence_end + pad (Episode 262: pause after "zed" → ~17.50).
 */
export function deriveT1FromSilenceIntervals(intervals, opts = {}) {
  const fromSec = Number(opts.fromSec ?? DEFAULT_T1_SEARCH.fromSec);
  const toSec = Number(opts.toSec ?? DEFAULT_T1_SEARCH.toSec);
  const minSilenceSec = Number(opts.minSilenceSec ?? DEFAULT_T1_SEARCH.minSilenceSec);
  const padAfterSilenceSec = Number(
    opts.padAfterSilenceSec ?? DEFAULT_T1_SEARCH.padAfterSilenceSec,
  );
  if (!(toSec > fromSec)) {
    throw new Error("T1 search window requires toSec > fromSec");
  }
  const candidates = (intervals || []).filter((iv) => {
    if (!iv || !Number.isFinite(iv.start) || !Number.isFinite(iv.end)) return false;
    if (iv.duration < minSilenceSec) return false;
    // Silence must intersect the window and end near/inside it.
    const overlaps = iv.end >= fromSec && iv.start <= toSec;
    const endsInWindow = iv.end >= fromSec && iv.end <= toSec + 0.5;
    return overlaps && endsInWindow;
  });
  if (candidates.length === 0) {
    return {
      ok: false,
      t1: null,
      reason: `no silence >= ${minSilenceSec}s ending in [${fromSec}, ${toSec}]`,
      candidates: [],
    };
  }
  candidates.sort((a, b) => b.duration - a.duration || a.end - b.end);
  const chosen = candidates[0];
  const t1 = Number((chosen.end + padAfterSilenceSec).toFixed(3));
  return {
    ok: true,
    t1,
    reason: `silence ${chosen.start.toFixed(3)}-${chosen.end.toFixed(3)}s (+${padAfterSilenceSec}s pad)`,
    candidates,
    chosen,
  };
}

export function resolveCutTimes({ durationSec, t1, t2, t2Fraction = DEFAULT_T2_FRACTION }) {
  const D = Number(durationSec);
  if (!Number.isFinite(D) || D <= 1) {
    throw new Error("Sarah master duration must be a number > 1");
  }
  const T1 = Number(t1);
  if (!Number.isFinite(T1) || T1 <= 0 || T1 >= D - 0.5) {
    throw new Error(`T1 must be in (0, D-0.5); got ${t1} with D=${D}`);
  }
  let T2 = t2 == null || t2 === "" ? D * Number(t2Fraction) : Number(t2);
  if (!Number.isFinite(T2)) {
    throw new Error(`T2 must be a number; got ${t2}`);
  }
  T2 = Number(T2.toFixed(4));
  if (T2 <= T1 + 0.5 || T2 >= D - 0.2) {
    throw new Error(`T2 must satisfy T1+0.5 < T2 < D-0.2; got T1=${T1} T2=${T2} D=${D}`);
  }
  return {
    D,
    T1: Number(T1.toFixed(3)),
    T2,
    durB: Number((T2 - T1).toFixed(4)),
    durC: Number((D - T2).toFixed(4)),
    t2FromFraction: t2 == null || t2 === "",
    t2Fraction: Number(t2Fraction),
  };
}

/**
 * Plan Part B picture: motion screenshare then optional still cutaway.
 * cutawaySeconds is taken from the end of the mid section (Episode 262: ~6.3s).
 */
export function planMidBeats({ durB, cutawaySeconds = 0, cutaways = [], screenshareDurationSec }) {
  const mid = Number(durB);
  if (!Number.isFinite(mid) || mid <= 0) {
    throw new Error("durB must be a positive number");
  }
  const plannedCutaways =
    cutaways.length > 0
      ? cutaways.map((cutaway, index) => {
          const durationSec = Number(cutaway?.durationSec);
          if (!Number.isFinite(durationSec) || durationSec <= 0) {
            throw new Error(`cutaway ${cutaway?.id || index + 1} durationSec must be > 0`);
          }
          return {
            kind: "cutaway",
            id: cutaway?.id || `cutaway-${index + 1}`,
            durationSec,
          };
        })
      : Number(cutawaySeconds) > 0
        ? [{ kind: "cutaway", id: "cutaway-1", durationSec: Number(cutawaySeconds) }]
        : [];
  const cutaway = plannedCutaways.reduce((total, beat) => total + beat.durationSec, 0);
  if (cutaway <= 0) {
    return {
      beats: [{ kind: "screenshare", durationSec: mid }],
      screenshareUseSec: mid,
      cutawayUseSec: 0,
    };
  }
  if (cutaway >= mid - 0.25) {
    throw new Error(
      `--cutaway-seconds (${cutaway}) must leave at least 0.25s of screenshare in mid (${mid}s)`,
    );
  }
  const welcomeSec = Number((mid - cutaway).toFixed(4));
  const ssDur = Number(screenshareDurationSec);
  if (Number.isFinite(ssDur) && ssDur > 0 && welcomeSec > ssDur + 0.05) {
    throw new Error(
      `screenshare duration ${ssDur}s is shorter than mid motion need ${welcomeSec}s; ` +
        "trim T2, raise cutaway seconds, or supply a longer screenshare",
    );
  }
  return {
    beats: [{ kind: "screenshare", durationSec: welcomeSec }, ...plannedCutaways],
    screenshareUseSec: welcomeSec,
    cutawayUseSec: cutaway,
  };
}

export function parseCutawayManifest(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("cutaway manifest must be a JSON object");
  }
  if (value.version !== 1) {
    throw new Error("cutaway manifest version must be 1");
  }
  if (!Array.isArray(value.cutaways) || value.cutaways.length === 0) {
    throw new Error("cutaway manifest cutaways must be a non-empty array");
  }
  const seen = new Set();
  const cutaways = value.cutaways.map((entry, index) => {
    const id = typeof entry?.id === "string" ? entry.id.trim() : "";
    const file = typeof entry?.file === "string" ? entry.file.trim() : "";
    const durationSec = Number(entry?.durationSec);
    if (!/^[a-z0-9][a-z0-9._-]*$/i.test(id)) {
      throw new Error(
        `cutaway ${index + 1} id must use letters, numbers, dot, dash, or underscore`,
      );
    }
    if (seen.has(id)) throw new Error(`duplicate cutaway id: ${id}`);
    seen.add(id);
    if (!file || file.startsWith("/") || file.split(/[\\/]/).includes("..")) {
      throw new Error(`cutaway ${id} file must stay relative to the manifest directory`);
    }
    if (!Number.isFinite(durationSec) || durationSec <= 0) {
      throw new Error(`cutaway ${id} durationSec must be > 0`);
    }
    return {
      id,
      file,
      durationSec,
      note: typeof entry.note === "string" ? entry.note : undefined,
    };
  });
  return {
    version: 1,
    episode: value.episode == null ? undefined : String(value.episode),
    cutaways,
  };
}

export function selectCutaways(manifest, selectedIds) {
  const ids = selectedIds.map((id) => String(id).trim()).filter(Boolean);
  if (ids.length === 0) return [];
  if (new Set(ids).size !== ids.length) throw new Error("cutaway selections must be unique");
  const byId = new Map(manifest.cutaways.map((cutaway) => [cutaway.id, cutaway]));
  return ids.map((id) => {
    const cutaway = byId.get(id);
    if (!cutaway) {
      throw new Error(
        `unknown cutaway id: ${id} (available: ${manifest.cutaways.map((item) => item.id).join(", ")})`,
      );
    }
    return cutaway;
  });
}

/** Absolute times on the assembled RC timeline to sample for Finder QC. */
export function planCutPointQcTimes(T2, offsetsSec = DEFAULT_QC_OFFSETS_SEC) {
  const t2 = Number(T2);
  if (!Number.isFinite(t2)) throw new Error("T2 must be a number for QC");
  return [...offsetsSec].map((off) => {
    const n = Number(off);
    let label;
    if (n === 0) label = "T2";
    else if (Math.abs(n + 1 / 24) < 1e-9) label = "T2-1frame";
    else label = `T2${n > 0 ? "+" : ""}${n}`;
    return {
      offsetSec: n,
      atSec: Number((t2 + n).toFixed(3)),
      label,
    };
  });
}

/**
 * Evaluate sampled frame stats near T2 against a mid-section reference.
 * Returns { ok, failures[] }.
 */
export function evaluateCutPointQc({ referenceStats, samples }) {
  if (!referenceStats) {
    return { ok: false, failures: [{ reason: "missing reference frame stats" }] };
  }
  const failures = [];
  for (const sample of samples || []) {
    if (!sample?.stats) {
      failures.push({
        atSec: sample?.atSec,
        label: sample?.label,
        reason: "could not sample frame",
      });
      continue;
    }
    if (looksLikeDesktopOrFinderTail(sample.stats, referenceStats)) {
      failures.push({
        atSec: sample.atSec,
        label: sample.label,
        reason: "Finder/Desktop-like pixels near cut",
        stats: sample.stats,
      });
    }
  }
  return { ok: failures.length === 0, failures };
}

/**
 * Strip repo transcript chrome to spoken words for Desktop paste comparison.
 */
export function extractSpokenBody(markdownOrPlain) {
  let text = String(markdownOrPlain || "");
  // Drop YAML/front-matter style blocks if present.
  text = text.replace(/^---[\s\S]*?---\s*/m, "");
  // Prefer content after the first thematic break when present (repo transcripts).
  const parts = text.split(/\n---\n/);
  if (parts.length > 1) {
    text = parts.slice(1).join("\n---\n");
  }
  text = text
    .replace(/^\*\*Sarah:\*\*\s*/gim, "")
    .replace(/^Sarah:\s*/gim, "")
    .replace(/^#+.*$/gm, "")
    .replace(/^\[[^\]]+\]:\s*.*$/gm, "")
    .replace(/^\s*>\s?/gm, "");
  return normalizeSpokenText(text);
}

export function normalizeSpokenText(text) {
  return String(text || "")
    .replace(/\r\n/g, "\n")
    .replace(/[“”]/g, '"')
    .replace(/[‘’]/g, "'")
    .replace(/[—–]/g, "-")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

/**
 * Compare Desktop paste transcript to repo docs/transcripts spoken body.
 */
export function checkTranscriptLock({ desktopText, repoText }) {
  const desktop = normalizeSpokenText(desktopText);
  const repo = extractSpokenBody(repoText);
  if (!desktop) {
    return { ok: false, reason: "desktop transcript is empty", desktop, repo };
  }
  if (!repo) {
    return { ok: false, reason: "repo transcript spoken body is empty", desktop, repo };
  }
  if (desktop === repo) {
    return { ok: true, reason: "desktop paste matches repo spoken body", desktop, repo };
  }
  // Soft mismatch detail for operators.
  const max = Math.min(desktop.length, repo.length);
  let i = 0;
  while (i < max && desktop[i] === repo[i]) i += 1;
  return {
    ok: false,
    reason: `spoken text drift at char ${i}`,
    desktop,
    repo,
    desktopLen: desktop.length,
    repoLen: repo.length,
  };
}

/** Build ffmpeg filter_complex for A/B/C with continuous Sarah audio. */
export function buildAssembleFilterComplex({
  T1,
  T2,
  D,
  midPlan,
  sarahLabel = "0",
  screenshareLabel = "1",
  cutawayLabel = "2",
  cutawayLabels,
  hasCutaway = false,
}) {
  const t1 = Number(T1).toFixed(4);
  const t2 = Number(T2).toFixed(4);
  const d = Number(D).toFixed(4);
  const parts = [];
  // Sarah audio end-to-end.
  parts.push(`[${sarahLabel}:a]atrim=0:${d},asetpts=PTS-STARTPTS[aout]`);
  // Part A picture.
  parts.push(`[${sarahLabel}:v]trim=0:${t1},setpts=PTS-STARTPTS[va]`);
  const screenshareBeat = midPlan.beats.find((b) => b.kind === "screenshare");
  const cutawayBeats = midPlan.beats.filter((b) => b.kind === "cutaway");
  const ssDur = Number(screenshareBeat.durationSec).toFixed(4);
  if (hasCutaway && cutawayBeats.length > 0) {
    parts.push(`[${screenshareLabel}:v]trim=0:${ssDur},setpts=PTS-STARTPTS[vb1]`);
    const labels =
      cutawayLabels || cutawayBeats.map((_, index) => String(Number(cutawayLabel) + index));
    cutawayBeats.forEach((beat, index) => {
      const durationSec = Number(beat.durationSec).toFixed(4);
      parts.push(
        `[${labels[index]}:v]trim=0:${durationSec},setpts=PTS-STARTPTS,fps=24,format=yuv420p[vb${index + 2}]`,
      );
    });
    const inputs = ["[vb1]", ...cutawayBeats.map((_, index) => `[vb${index + 2}]`)].join("");
    parts.push(`${inputs}concat=n=${cutawayBeats.length + 1}:v=1:a=0[vb]`);
  } else {
    parts.push(`[${screenshareLabel}:v]trim=0:${ssDur},setpts=PTS-STARTPTS[vb]`);
  }
  parts.push(`[${sarahLabel}:v]trim=${t2}:${d},setpts=PTS-STARTPTS[vc]`);
  parts.push("[va][vb][vc]concat=n=3:v=1:a=0[vout]");
  return parts.join(";");
}

export function parseQcOffsetsFlag(raw) {
  if (raw == null || raw === "") return [...DEFAULT_QC_OFFSETS_SEC];
  const parts = String(raw)
    .split(",")
    .map((s) => Number(s.trim()));
  if (!parts.every((n) => Number.isFinite(n))) {
    throw new Error("--qc-offsets must be a comma-separated list of numbers");
  }
  return parts;
}

export function parseNumberFlag(raw, name) {
  if (raw == null || raw === "") return null;
  const n = Number(raw);
  if (!Number.isFinite(n)) throw new Error(`${name} must be a number`);
  return n;
}
