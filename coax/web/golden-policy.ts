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
