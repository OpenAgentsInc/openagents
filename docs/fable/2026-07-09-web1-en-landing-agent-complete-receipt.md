# WEB-1-EN landing — agent-complete conversion receipt

Issue: OpenAgentsInc/openagents#8595

Route: `/landing-en` in `apps/openagents.com/apps/start` (TanStack Start).

**This is not a root cutover.** `/` and `/new` stay untouched. Root-flip and
final landing copy remain owner gates on #8595 (formerly held by #8565).

## Agent-complete status (this receipt)

The landing is fully authored as **one typed Effect Native view tree** from
the vendored marketing catalog (`effect-native/v26`). React is only the thin
route-shell host that mounts the tree through `makeDomRenderer` (EN adapter
rule). There are zero React section components in the landing content.

| Section | Catalog component(s) | Live data |
| --- | --- | --- |
| Navbar | `NavBar` | — |
| Announcement | `AnnouncementBadge` | — |
| Hero + mockup/glow | `Hero` + `Glow` + `MockupFrame` | — |
| Logos / surfaces | `LogoRow` (https URI placeholders; see gaps) | — |
| Items / features | `Section` + `Card` | — |
| Stats | `StatsBand` | LIVE public counters |
| Pricing | `PricingTable` / `PricingColumn` | LIVE Khala Code plan catalog |
| FAQ | `Accordion` (`LandingEnFaqToggled`) | — |
| CTA | `CtaSection` + `Glow` | — |
| Footer | `Footer` | — |

- **Theme:** canonical `khalaTheme` from `@effect-native/tokens`.
- **Live path:** same fail-soft public-projection fetchers as `/stage1`
  (`fetchKhalaTokensServed`, `fetchPylonStats`, `fetchKhalaCodePlans`). Pending
  and unavailable states stay explicit; no number is fabricated.
- **Copy freeze:** existing OpenAgents strings + `TODO(owner-copy)` placeholders
  preserved verbatim. Launch-ui template author copy must not ship here.
- **Source boundary:** no `@/components/launch-ui` or `lucide-react` in the EN
  page or route file.

## What agents finished (vs what still needs the owner)

### Done on the agent side

1. Full marketing-catalog authoring of every landing section at `/landing-en`.
2. `LogoRow` wired via URI-schema-legal `https://` placeholders (stage1 pattern);
   alts keep the four OpenAgents surface names.
3. Accordion FAQ, live StatsBand, live PricingTable already in tree; tests pin
   marketing tags, hydration, fail-soft unavailable pricing, DOM mount markers,
   and the no-launch-ui boundary.
4. Side-by-side baseline `/new` left intact for visual comparison.

### Owner residual (do not close #8595 without these)

1. **Final landing copy sign-off** — replace `TODO(owner-copy)` FAQ/CTA strings
   and confirm hero/suite/footer wording (copy is owner-gated).
2. **Root-flip decision** — whether `/` (or production entry) mounts the EN
   landing instead of the React replica; needs an explicit rollback note when
   that happens. **Agents must not flip the root route without that gate.**
3. **Delete-as-you-replace** — remove React launch-ui landing sections only
   after owner sign-off of the flip (#8595 / EN-4 convert-and-delete).
4. **Brand logo assets** — optional: replace simpleicons CDN placeholders with
   owner-approved brand/partner image URLs (typed tree shape unchanged).

### Known EN/catalog gaps (not monorepo patches)

Tracked on EN-2 (#8572) and related catalog issues; do not vendor-hack around
them in `apps/start`:

- `Image` / `LogoRow.source` require an absolute URI scheme (no same-origin
  relative `/dashboard-dark.png`).
- Launch-ui-grade pixel polish (angled mockup, gradient text-clip, denser
  spacing, glow bloom) is renderer styling in the catalog.
- `StatsBand` has no per-stat description slot.

## Verification

```bash
bun run --cwd apps/openagents.com/apps/start test -- src/routes/-landing-en.test.tsx
```

## Related

- Prior EN-1 stage1 receipt: `docs/fable/2026-07-08-en-1-stage1-effect-native-receipt.md`
- EN-4 web absorption burn-down: `docs/effect-native/2026-07-09-web-absorption-burndown.md`
- Umbrella / owner gates: OpenAgentsInc/openagents#8595
