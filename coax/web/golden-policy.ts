// What the release walk (goldens.ts) does with a page that has no golden yet.
// A realm the galaxy doesn't list yet (realms.json listed: false) may arrive
// before its golden: it must still start, show no errors and draw (goldens.ts
// checks those first), and its shot waits in goldens/pending/ for approval.
// A listed realm, and any page that once had a golden, stays strict.
export type Missing = { fail: boolean; pending: string; line: string }

export function missingGolden(name: string, unlisted: boolean, pendingDir: string, base: string): Missing {
  const pending = `${pendingDir.replace(/\/$/, "")}/${name}.png`
  if (!unlisted) return { fail: true, pending: "", line: `${name}: no golden (coax/web/goldens/${name}.png); run --update and look at it` }
  return { fail: false, pending,
    line: `NEW REALM ${name}: no golden yet, approve with \`bun coax/web/goldens.ts ${base} --update\`, look at ${pending} and commit coax/web/goldens/${name}.png` }
}

// A walk's failures, for release.sh's hold-back: golden, the pages whose
// golden comparison failed (or that have none and must), each with its path;
// other, every other failure (doesn't start, errors, draws nothing, the
// analytics contract), which no release ships past. A page in both is held
// back and still stops the release, through its other failure.
export type Failures = { golden: { name: string; path: string }[]; other: string[] }

export function classify(failed: string[], pages: Record<string, { path: string }>): Failures {
  const golden = new Map<string, string>(), other: string[] = []
  for (const f of failed) {
    const m = /^([a-z0-9.-]+): (matches its golden \(|no golden )/.exec(f)
    if (m && pages[m[1]]) golden.set(m[1], pages[m[1]].path); else other.push(f)
  }
  return { golden: [...golden].map(([name, path]) => ({ name, path })), other }
}
