import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { LoggedOut } from '../../../model'
import { HomeRoute } from '../../../route'
import {
  RequestedPollKhalaTokensServed,
  RequestedPollKhalaTokensServedHistory,
  SucceededLoadPublicKhalaTokensServed,
  SucceededLoadPublicKhalaTokensServedHistory,
} from '../message'
import {
  IdlePublicForumLaunchStatus,
  IdlePublicForumTipLeaderboards,
  IdlePublicKhalaTokensServed,
  IdlePublicKhalaTokensServedHistory,
  LoadedPublicKhalaTokensServed,
  LoadedPublicKhalaTokensServedHistory,
  LoadingPublicKhalaTokensServedHistory,
  LoadingPublicPylonStats,
  PublicKhalaTokensServed,
  PublicKhalaTokensServedHistory,
  init,
  initSettledFeedModel,
} from '../model'
import { update } from '../update'
import * as Home from './home'
import * as StatsPage from './stats'

const served = (tokensServed: number) =>
  PublicKhalaTokensServed.make({
    tokensServed,
    generatedAt: '2026-06-24T12:00:00.000Z',
  })

const sampleHistory = PublicKhalaTokensServedHistory.make({
  window: '30d',
  bucket: 'day',
  series: [
    { day: '2026-06-20', tokensServed: 12_000 },
    { day: '2026-06-21', tokensServed: 48_500 },
    { day: '2026-06-22', tokensServed: 0 },
    { day: '2026-06-23', tokensServed: 96_250 },
    { day: '2026-06-24', tokensServed: 31_000 },
  ],
})

// The counter is independent of Pylon stats, so the surrounding strip can stay
// in its Loading state (rendered as "Unavailable") with no heavy fixture.
const homeInputWithTokens = (tokensServed: number) => ({
  forumLaunchStatus: IdlePublicForumLaunchStatus(),
  forumTipLeaderboards: IdlePublicForumTipLeaderboards(),
  publicKhalaTokensServed: LoadedPublicKhalaTokensServed({
    served: served(tokensServed),
  }),
  publicKhalaTokensServedHistory: LoadedPublicKhalaTokensServedHistory({
    history: sampleHistory,
  }),
  publicPylonStats: LoadingPublicPylonStats(),
  settledFeed: initSettledFeedModel(),
})

describe('Khala Tokens Served counter (#6227)', () => {
  test('a poll tick issues the public-read command without flashing the model', () => {
    const [model, commands] = update(
      init(HomeRoute()),
      RequestedPollKhalaTokensServed(),
    )

    // The poll holds the model as-is (no Loading flash) and queues one command.
    expect(model.publicKhalaTokensServed._tag).toBe(
      'PublicKhalaTokensServedLoading',
    )
    expect(commands).toHaveLength(1)
  })

  test('a succeeded load stores the served total in the loaded state', () => {
    const [model] = update(
      init(HomeRoute()),
      SucceededLoadPublicKhalaTokensServed({ served: served(1_250_000) }),
    )

    expect(model.publicKhalaTokensServed._tag).toBe(
      'PublicKhalaTokensServedLoaded',
    )
    if (
      model.publicKhalaTokensServed._tag === 'PublicKhalaTokensServedLoaded'
    ) {
      expect(model.publicKhalaTokensServed.served.tokensServed).toBe(1_250_000)
    }
  })

  test('renders the single fixed label and the formatted live total', () => {
    Scene.scene(
      {
        update,
        view: () => Home.view(homeInputWithTokens(1_250_000)),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('Khala Tokens Served')).toExist(),
      Scene.expect(Scene.text('1,250,000')).toExist(),
    )
  })

  test('renders the label even before the first total loads', () => {
    Scene.scene(
      {
        update,
        view: () =>
          Home.view({
            forumLaunchStatus: IdlePublicForumLaunchStatus(),
            forumTipLeaderboards: IdlePublicForumTipLeaderboards(),
            publicKhalaTokensServed: IdlePublicKhalaTokensServed(),
            publicKhalaTokensServedHistory:
              IdlePublicKhalaTokensServedHistory(),
            publicPylonStats: LoadingPublicPylonStats(),
            settledFeed: initSettledFeedModel(),
          }),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('Khala Tokens Served')).toExist(),
    )
  })

  test('counts up: the keyed odometer value changes between poll totals', () => {
    // The odometer re-fires its keyframe by REMOUNTING — the number span is
    // keyed on the formatted value, so a new poll total produces a new key and
    // the count-up animation runs. Render two totals and assert the rendered
    // markup carries the odometer class and the two distinct formatted numbers.
    const first = JSON.stringify(Home.view(homeInputWithTokens(15)))
    const later = JSON.stringify(Home.view(homeInputWithTokens(150)))

    expect(first).toContain('oa-odometer-number')
    expect(first).toContain('15')
    expect(later).toContain('150')
    expect(first).not.toEqual(later)
  })
})

describe('Khala Tokens Served history chart (#6227)', () => {
  test('a history poll tick issues the public-read command without flashing', () => {
    const [model, commands] = update(
      init(HomeRoute()),
      RequestedPollKhalaTokensServedHistory(),
    )

    // The poll holds the history model as-is (no Loading flash) and queues one
    // command.
    expect(model.publicKhalaTokensServedHistory._tag).toBe(
      'PublicKhalaTokensServedHistoryLoading',
    )
    expect(commands).toHaveLength(1)
  })

  test('a succeeded load stores the series in the loaded state', () => {
    const [model] = update(
      init(HomeRoute()),
      SucceededLoadPublicKhalaTokensServedHistory({ history: sampleHistory }),
    )

    expect(model.publicKhalaTokensServedHistory._tag).toBe(
      'PublicKhalaTokensServedHistoryLoaded',
    )
    if (
      model.publicKhalaTokensServedHistory._tag ===
      'PublicKhalaTokensServedHistoryLoaded'
    ) {
      expect(model.publicKhalaTokensServedHistory.history.series).toHaveLength(5)
    }
  })

  test('renders the chart heading and an accessible per-day text fallback', () => {
    Scene.scene(
      {
        update,
        view: () => Home.view(homeInputWithTokens(1_250_000)),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('Tokens Served / Day')).toExist(),
      // The visually-hidden fallback lists each day + value, so the data is
      // never locked inside the SVG.
      Scene.expect(Scene.text('2026-06-23: 96,250 tokens')).toExist(),
    )
  })

  test('renders the same per-day curve on public /stats', () => {
    Scene.scene(
      {
        update,
        view: () => StatsPage.view(homeInputWithTokens(1_250_000)),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('Network Stats')).toExist(),
      Scene.expect(Scene.text('Tokens Served / Day')).toExist(),
      Scene.expect(Scene.text('2026-06-23: 96,250 tokens')).toExist(),
    )
  })

  test('renders a graceful empty state when the series is empty', () => {
    const emptyHistory = PublicKhalaTokensServedHistory.make({
      window: '30d',
      bucket: 'day',
      series: [],
    })

    Scene.scene(
      {
        update,
        view: () =>
          Home.view({
            forumLaunchStatus: IdlePublicForumLaunchStatus(),
            forumTipLeaderboards: IdlePublicForumTipLeaderboards(),
            publicKhalaTokensServed: IdlePublicKhalaTokensServed(),
            publicKhalaTokensServedHistory: LoadedPublicKhalaTokensServedHistory(
              { history: emptyHistory },
            ),
            publicPylonStats: LoadingPublicPylonStats(),
            settledFeed: initSettledFeedModel(),
          }),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('No tokens served yet.')).toExist(),
    )
  })

  test('renders a loading state before the first series loads', () => {
    Scene.scene(
      {
        update,
        view: () =>
          Home.view({
            forumLaunchStatus: IdlePublicForumLaunchStatus(),
            forumTipLeaderboards: IdlePublicForumTipLeaderboards(),
            publicKhalaTokensServed: IdlePublicKhalaTokensServed(),
            publicKhalaTokensServedHistory:
              LoadingPublicKhalaTokensServedHistory(),
            publicPylonStats: LoadingPublicPylonStats(),
            settledFeed: initSettledFeedModel(),
          }),
      },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('Loading history…')).toExist(),
    )
  })
})
