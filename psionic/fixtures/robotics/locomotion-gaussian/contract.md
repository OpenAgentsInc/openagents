# Chapter171 next bounded component: normalized scalar Gaussian policy primitives

Root contract proposal, 2026-11-01. Before code, Data derives/pins the two inward log-domain endpoints and all peers review this document. Freeze that amendment before source implementation or expected generation. No tape assembly, learner, environment, optimizer, model or SDK in this component.

Work only in /Users/christopherdavid/.codex/worktrees/robotics-foundation-release/openagents, currently clean at published5b37237969. Leave the NEW checkout frozen. Existing counter-rng/tape/GAE/PPO bodies and passing gates remain untouched. Source additions are three authority roots with checks FIRST, zero actual new-root mutation survivors, no waivers. Use gaussian-types/fixtures, gaussian-density, gaussian-sampling, gaussian-ratio and a NEW load-only public locomotion-gaussian.coax; do not extend frozen locomotion.coax. Prefix Rgp/rgp for density/sampling and Rlr/rlr for ratio. Verify no collisions first.

## Numerical/coordinate profile

Darwin-arm64 CPU binary64, RN ties-to-even, gradual underflow, no contraction/fast reassociation. Coax language exp/log/sqrt/cos intrinsics use backend libm; this is not an own-libm accuracy proof. All formulas/policy decisions live in Coax. Normalized scalar action coordinates ONLY; physical action scaling/Jacobians are later work.

RgpAction nominal {value:f64} for action and mean. Log standard deviation, uniforms and numerical diagnostic/score values use Quantity(UnitOne,f64), with explicit raw adapters. All input values are finite BEFORE checking numeric domains. Mean in[-16,16], action for density in[-128,128], log_std in[-4,2]. All endpoints inclusive. Output shape EXACT1 always checked first. Admission is complete before arithmetic/publication. Nonfinite input=>number; finite out-of-domain=>range. Output unchanged on refusal, success staged before publication.

RgpDensity32B four ordered F64 values: logp,score_mean,score_logstd,entropy. RgpValue16B action,z. RgpDraw32B action,z,next(CrSeed twoU64). RgpStatus distinct enum u8 ok0/shape1/number2/range3/exhausted4. Raw wrappers add units/nominal roles and preserve admission/staging. Shared PODs are not provenance certificates.

## Density exact operation recipe

C = correctly rounded binary64(0.5*ln(2*pi)) =0.9189385332046727. H =RN(0.5+C). inv_sigma=exp(-log_std). d=RN(action-mean), except numerically equal action==mean canonicalizes d=+0. z=RN(d*inv_sigma). z2=RN(z*z). logp=RN(RN(RN(-0.5*z2)-log_std)-C). score_mean=RN(z*inv_sigma). score_logstd=RN(z2-1). entropy=RN(log_std+H). Equal action/mean therefore has exact +0 mean score and -1 log_std score. Nonzero subnormal results retain operation-recipe zero signs. These scores are formal smooth density derivatives for detached sampled action, not floating-point staircase derivatives. No autodiff system or pathwise plant derivative is added.

## Sampling exact operation recipe

Pure rgp_from_uniforms(mean,log_std,U_radial,U_angle,out1) admits both uniforms in[0,1). They are raw top53 values BEFORE the radial1-U transform. Both finite and domain checked with mean/log_std before effects. TWO_PI=correctly rounded binary64(2*pi)=6.283185307179586. w=RN(1-U_radial), theta=RN(TWO_PI*U_angle), radius=sqrt(RN(-2*log(w))), z=RN(radius*cos(theta)), sigma=exp(log_std), action=RN(mean+RN(sigma*z)). ONE scalar output, no cached spare.

Explicit radius-zero contract: if U_radial==0 (either zero sign), z=canonical+0 and action is a BYTE-EXACT copy of admitted mean, including negative zero. Still validate all inputs. This branch may bypass unused transcendental work. Otherwise follow recipe; no general zero canonicalization or clamp/retry. Uniform w has representable lower bound2^-53, giving a finite transformed tail. Quarter-turn oracle evaluates cos at the exact rounded theta, never assumes mathematical cos(pi/2)==0. Input hyper bounds ensure finite generated action inside the density action domain.

rgp_sample(seed:CrSeed BY VALUE,mean,log_std,out1) admits output shape, both hyper finite, hyper domains, THEN counter headroom counter<=UINT64_MAX-2. Failure=>exhausted without draw/publication. Call existing cr_draw EXACTLY twice, take first.value as U_radial and second.value as U_angle, and publish action,z,next=second.next together. Final counter may equal UINT64_MAX; subsequent draw refuses. No retries/spare/global state. This finite-top53, seeded numerical transform is not an exact continuous Gaussian or independence-quality proof. Existing CrSeed/cr_draw source/gates are carried, not edited/retested separately.

## Log-ratio to existing clipped objective

RlrStatus distinct u8 ok0/shape1/number2/range3. RlrOutput56B ordered log_ratio:F64Quantity,ratio:F64Quantity,objective:RcpOutput40B. Inputs new_logp/old_logp/epsilon UnitOne quantities, advantage RgaReward. Raw wrapper available. Shape1 first; ALL4 numeric fields finite next; advantage|A|<=1e9 and epsilon[0,1] next; then dlog=RN(new_logp-old_logp). If subtraction overflow or dlog outside the fixed inward log interval, range refusal before exp. No arbitrary cap on individual finite logp inputs.

Data MUST derive/pin lower=smallest binary64 >= exact ln(2^-20), upper=largest binary64 <= exact ln(16), using exact Fraction comparison to Decimal precision with stability evidence. Named inward-log profile deliberately distinguishes its log-domain admission from the direct-ratio API's rounded preimage. Exp only inside this safe finite interval; rho=exp(dlog). Require actual rho in[2^-20,16] and finite, otherwise range refusal; NEVER clamp it. Evaluate existing rcp_surrogate on rho/A/epsilon into private staged output; publish dlog,rho,objective only on success. No PPO root changes.

For oracle composition, verify independent high-precision exp(dlog) against a frozen tight transcendental budget, then verify ALL Rcp objective words/flags EXACTLY using Fraction/RN on the separately admitted ACTUAL emitted rho. This is a conditional composition proof, not circular exp validation; disclose it. Exact dlog bits/domain admission and explicit zero/logp0 cases stay exact. PPO knots are defined from the realized binary64 rho, not an ideal real ratio.

## Evidence/workload boundaries

Data freezes whole producer, corpus bytes/counts, local-operation recipe and conditioned budgets before product comparisons; FIRST expected generation only after review. Use independent Decimal80 exp/log/sqrt and independently reduced high-precision cosine; exact Fraction/RN local values. Refusal policy/shape/status/seed counters/zero contracts exact. Samples/density have frozen absolute+relative or conditioned forward budgets, not blanket bit-exact claims. Include endpoints, adjacent ULPs, signed zero/subnormal/NaN/Inf, safe density extremes, exact density-at-mean, rounded angle quarter turns, counter boundary, alias/atomic refusal and ratio domain/knot composition. Source mutation tests must independently constrain meaningful branches, not merely repeat product formulas.

Core first native three profiles ONLY new APIs/corpus, C transport/ABI/refusal effects only, no C policy/math. All observed and expected numerical fields finite before maxima. Preserve each passing mode immediately; no reruns of unchanged modes/gates. Root performs default-dispatch spies, book integration, current compiler C/H binding by carried private908 emitter, fetch/rebase and push. Existing compiler PRIVATE binary /tmp/coax-locomotion-current-compiler-first/coax SHAedcfc624f8313dde3a53786e2f91dfbb8ab072f74a7d9e48cdbe1635b90912d1 may be used for new source emission; no rebuild/full gates. Every command<=15min, actual native subcommands<=180s, component<=2h start-to-push.

## Sealed amendment after peer review

All three peers reviewed proposal447e9ec5…9c9c. Core and Flow found no blocking API/recipe issue. Data derived endpoints with Decimal80/160/240 and exact Fraction ordering, stable across all precisions:

- Lower decimal-13.862943611198906, binary64 bitsc02bb9d3beb8c86b (hex -0x1.bb9d3beb8c86bp+3).
- Upper decimal2.772588722239781, binary64 bits40062e42fefa39ef (hex 0x1.62e42fefa39efp+1).

The lower has extremely small real inward margin, so exp may legitimately produce rho just below2^-20 within the allowed libm error. A NEW test-only Coax intrinsic transport entry may export exp(dlog) for frozen explicitly in-interval/safe oracle rows. The native C harness reads a frozen probe-request bit and performs transport only; it does not compute exp or admission. The independent Decimal oracle verifies probe rho accuracy first; exact ratio-domain predicates then determine the CONDITIONAL expected public status. A successful public rho must equal the probe bytes, and all Rcp output words/flags must equal exact Fraction/RN composition on that rho. Invalid/out-of-interval cases do not request the probe. Probe code is not an oracle, public API, separate policy proof or a correctly-rounded-libm promise.

Corpus approved BEFORE generation/comparison: density960, pure-uniform1344, seeded160 and ratio640 rows. Data must freeze exact row partitions, bytes, source and any unavailable-output encoding before expected generation. Smooth field budgets: density4e-12+8e-15*abs(reference); pure/seed action and z8e-14+8e-15*abs(reference); ratio/probe rho4e-15*abs(reference). Entropy and every density-at-mean word, explicit radial0 action copy/z+0, seed/counter transitions, dlog and declared exact status/mask contracts are EXACT; ratio status/Rcp words are conditional on independently bounded observed probe rho as above. Do not change counts, budgets or domains after comparison results.

The pure-uniform API accepts all binary64[0,1), including tiny nonzero radial inputs whose RN(1-U)==1; only the SEEDED profile is top53. Those tiny nonzero inputs follow the NONZERO sqrt(-0)/cos recipe and must not be silently canonicalized as the U==0 branch.

Source VM/mutation/unit witnesses use the carried qualifieda429 binary. Native first emission uses existing private current908 directly, no compiler rebuild or duplicate emission contexts. Three native compile profiles add -fno-builtin-exp -fno-builtin-log -fno-builtin-sqrt -fno-builtin-cos to pin scalar backend calls. No full compiler, old source/native, model or SDK gates are repeated. This contract is now SEALED for implementation; checks precede each new body. Expected generation still needs frozen-producer/input review GO.

## Precomparison coefficient correction (Root)

The earlier stated C=.9189385332046727 was one binary64 ULP below the nearest value to exact0.5*ln(2*pi). Root independently evaluated Decimal90 with90-digit pi BEFORE any expected-generation or native comparison: exact C starts0.9189385332046727417803297364056176398613974736377834128. Correct nearest C=.9189385332046728, hex0x1.d67f1c864beb5p-1 (bits3fed67f1c864beb5); its error is+3.87829415806724e-17, versus−7.22393608818432e-17 for the original literal. Use CORRECTED C everywhere. H remains RN(.5+C). TWO_PI was independently checked and remains6.283185307179586 (hex0x1.921fb54442d18p+2).

Preserve Flow's first42-site passing density context under the old coefficient as NONFINAL history. Only coefficient-dependent source contexts require new proof; unrelated unchanged passing roots carry. No native or expected comparison had run, and no budgets/domains/other recipes change. This is correction of Root's mathematical constant, not tuning to product results.
