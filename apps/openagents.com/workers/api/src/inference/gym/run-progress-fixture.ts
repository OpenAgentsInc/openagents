// Seeded public-safe live Gym run progress (#6261).
//
// Until the live Hydralisk Harbor poll is wired, the `/gym` follow-along view and
// the run-progress endpoints render from this seeded fixture so the surface is
// honestly populated rather than empty or faked. Every object is built through
// `buildGymRunProgress`, so the public-safety boundary and count-consistency
// checks run at module load — a leak would throw on import, not at request time.
//
// The fixture intentionally includes BOTH publication states so the public
// projection's honest-degradation path is always exercised on the live surface:
//   - a `web_authorized` partial run (renders live counts + in-progress label),
//   - a `local_only` run (degrades to awaiting-authorization on the public path).
import { buildGymRunProgress, type GymRunProgress } from './run-progress'

const webAuthorizedPartial: GymRunProgress = buildGymRunProgress({
  runRef: 'run.gym.terminal_bench.glm_reap_mtp2.live.fixture',
  jobRef: 'job.gym.harbor_terminal_bench.glm_reap_mtp2.fixture',
  configId: 'gym.terminal_bench.glm_reap_mtp2.fixture',
  profileRef: 'glm-reap-504b-g4-tp4-mtp2-rp105',
  agent: 'terminus-2',
  phase: 'running',
  publication: 'web_authorized',
  officialDenominator: 89,
  completedPassed: 27,
  completedFailed: 14,
  running: 4,
  pending: 44,
  error: 0,
  cancelled: 0,
  promptTokens: 1_840_000,
  completionTokens: 612_000,
  elapsedMs: 1_920_000,
  lastUpdatedAt: '2026-06-25T00:00:00.000Z',
  caveatRefs: [
    'caveat.gym.run_progress.partial_denominator_not_final_score',
    'caveat.gym.terminal_bench.mtp2_vllm_min_p_disabled',
  ],
  blockerRefs: [
    'blocker.gym.run_progress.decision_grade_report_requires_owner_armed_sweep',
  ],
})

const localOnlyRun: GymRunProgress = buildGymRunProgress({
  runRef: 'run.gym.terminal_bench.khala_heuristic.local.fixture',
  jobRef: 'job.gym.harbor_terminal_bench.khala_heuristic.fixture',
  configId: 'gym.terminal_bench.khala_heuristic.fixture',
  profileRef: 'khala-public-heuristic',
  agent: 'opencode',
  phase: 'running',
  publication: 'local_only',
  officialDenominator: 89,
  completedPassed: 9,
  completedFailed: 3,
  running: 2,
  pending: 75,
  error: 0,
  cancelled: 0,
  promptTokens: null,
  completionTokens: null,
  elapsedMs: 540_000,
  lastUpdatedAt: '2026-06-25T00:00:00.000Z',
  caveatRefs: ['caveat.gym.run_progress.partial_denominator_not_final_score'],
  blockerRefs: [],
})

export const LIVE_GYM_RUN_PROGRESS_FIXTURE: ReadonlyArray<GymRunProgress> = [
  webAuthorizedPartial,
  localOnlyRun,
]
