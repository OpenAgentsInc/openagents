# Research Note

This is a research document about another company, VIDA, and the particular
referral model described in public material about that product. It is not an
OpenAgents policy, payment contract, compliance decision, or runtime
implementation plan by itself. The useful idea to study is the product-native,
usage-funded referral structure: rewards are tied to real paid activity rather
than signups alone, and referral economics are modeled as a share of platform
fees or revenue rather than as detached marketing spend.

# VIDA Referral Model for OpenAgents

## Overview
VIDA.live, later associated with vida.global and vida.page, was built as a Bitcoin and Lightning-powered communications platform that let users charge for calls, messages, and livestream access in satoshis.[web:110][web:111][web:115] Public interviews with founder Lyle Pratt describe the product as a way to monetize time and attention while reducing spam through pricing and instant settlement over Lightning.[web:123][web:168]

This document summarizes the referral structure described publicly around VIDA and translates the useful parts into a design pattern that OpenAgents can adapt. Some details of the exact payout math are described publicly at a high level, but not all implementation specifics are fully documented in accessible sources, so the recommendation below focuses on the parts that are clearly evidenced and operationally useful.[web:111][web:168]

## Product Context
Users on VIDA could set prices for inbound communication and receive sats when someone called, messaged, or joined a paid stream.[web:110][web:115] Third-party explainers describe the platform fee as 20 percent by default, with lower fees available when users connected their own Lightning wallet, showing that the business model was built around taking a cut of communication payments rather than charging a flat subscription alone.[web:115]

That context matters because the referral system was not separate from the product. It was tied to transaction flow, which meant referral rewards could be funded from real usage instead of from a detached marketing budget.[web:111][web:115]

## Referral Structure
Public discussion by Lyle Pratt describes a two-layer model with both tier-one and tier-two referrals, meaning a user could earn from direct referrals and also from the referrals brought in by those direct referrals.[web:168] In the interview notes and public description, this was framed as "referrals of your referrals," which is the clearest available description of the second layer.[web:168]

Attribution appears to have been simple and product-native. Public material indicates that referral credit could be established through a signup path, a referral link, or a user's first meaningful contact inside the app, rather than through a heavy manual affiliate workflow.[web:115][web:168] That simplicity is one of the strongest ideas to preserve.

## Funding Source for Rewards
The most important structural insight is that VIDA appears to have funded partner rewards from its own platform fees. The Bitcoin Manual's coverage says that 90 percent of network fees were recycled into the Citadel Partners program, while the broader public discussion around VIDA's partner system describes platform fees being routed back into partner rewards to drive growth.[web:111][web:168]

This matters because it made the program usage-funded rather than subsidy-funded. In practical terms, every paid communication event generated revenue for VIDA and also created a pool from which referral incentives could be paid.[web:111][web:115]

## What Was Strong About the Model
The first strong feature was direct alignment with revenue. Referral payouts were connected to actual calls, messages, and streams rather than vanity metrics like raw signups.[web:110][web:115] That reduces fake growth and gives the company tighter control over unit economics.[web:111]

The second strong feature was speed. VIDA's communications product and public positioning emphasized real-time Lightning settlement, which made the referral loop feel immediate and legible to users who were already earning in sats from platform activity.[web:110][web:123] Fast payout reinforces participation more effectively than delayed monthly affiliate reporting.[web:115]

The third strong feature was low-friction attribution. Because the system appears to have relied on direct links and first-contact style attribution, it avoided much of the complexity common in enterprise affiliate stacks.[web:115][web:168] That kind of lightweight logic is especially relevant for agent ecosystems where onboarding has to happen quickly.

## Limits and Risks
The main evidence gap is that exact payout percentages by tier are not clearly documented in the publicly accessible material reviewed here. The existence of a two-layer structure is well supported, but the precise split between tier one, tier two, and the platform treasury is not fully recoverable from the available sources.[web:111][web:168]

A second risk is perception. Any multi-tier system can look like network marketing if rewards are based on recruiting alone, so the defensible design choice is to pay only on real platform usage and not on mere account creation.[web:111][web:115] VIDA's model appears strongest when interpreted as revenue-sharing on paid communications rather than as pure recruitment incentives.[web:110][web:168]

## OpenAgents Adaptation
For OpenAgents, the cleanest adaptation is to use the VIDA pattern as a two-layer revenue-share system on real agent activity. A user or node that directly brings in a new participant becomes the tier-one referrer, and that referrer's referrer becomes tier two.[web:168] Rewards should trigger only when the referred party generates paid activity such as agent jobs, task completions, paid routing, or marketplace transactions.

OpenAgents can improve on VIDA by using Nostr-native identity and credit attribution instead of cookies alone. Referral attribution can be attached to a pubkey, an invite token, or a signed event at onboarding, making the attribution durable, inspectable, and portable across the network in a way that aligns with OpenAgents' architecture priorities.[file:19]

A strong default policy would look like this:

- Tier 1 earns the larger share of referral rewards because they created the direct relationship.
- Tier 2 earns a smaller upstream share to encourage network expansion without overwhelming margins.
- Rewards are paid only from actual OpenAgents platform fees, never from treasury emissions disconnected from usage.
- Attribution is locked at first verified onboarding event or first funded job, reducing disputes.
- Payouts are made in sats or ecash equivalents as close to real time as operationally practical.

## Suggested Launch Design
A practical OpenAgents launch version would keep the system intentionally narrow at first. Start with one invite link per user or partner, one permanent tier-one relationship, one optional tier-two upstream relationship, and rewards only on a short list of monetized actions such as successful paid agent runs or completed paid sessions.[web:168][file:19]

The simplest rollout is:

| Component | Recommended OpenAgents version |
|---|---|
| Attribution event | First signed onboarding event or first funded job linked to inviter pubkey[web:168][file:19] |
| Reward source | Fixed share of OpenAgents platform fees[file:19] |
| Tier 1 | Main reward recipient for direct referrals[web:168] |
| Tier 2 | Smaller upstream recipient for referrals of referrals[web:168] |
| Payout unit | Sats or ecash equivalent consistent with Bitcoin-only rails[file:19] |
| Anti-abuse rule | No rewards for unfunded signups; rewards begin only after real paid activity[web:110][web:115] |

## Recommendation
The part of VIDA worth copying is not the branding or exact language but the structure: product-native attribution, two-layer upstream logic, and rewards funded from real transaction fees.[web:111][web:115][web:168] For OpenAgents, that means a Bitcoin-only, usage-funded, Nostr-attributed referral system where participants earn only when the users or agents they bring in generate real economic activity.[file:19]

That gives Chris a framework that is simple enough to ship quickly, legible to users, and much easier to defend than a broad multi-level program. It preserves the growth advantages of VIDA's design while fitting OpenAgents' agent network, Bitcoin-only payments, and sovereign identity direction.[web:111][web:168][file:19]
