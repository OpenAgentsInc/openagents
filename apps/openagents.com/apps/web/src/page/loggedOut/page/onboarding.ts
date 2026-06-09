import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { homeRouter } from '../../../route'
import * as Ui from '../../../ui'
import {
  type Message,
  ToggledOnboardingCoupon,
  UpdatedOnboardingCouponCode,
  UpdatedOnboardingFundingAmount,
} from '../message'
import type { OnboardingModel } from '../model'

const githubLoginHref = '/login/github'

const bonusForAmount = (amount: number): number => {
  if (amount >= 250) {
    return Math.round(amount * 0.15)
  }

  if (amount >= 100) {
    return Math.round(amount * 0.1)
  }

  if (amount >= 50) {
    return Math.round(amount * 0.05)
  }

  return 0
}

const dollars = (amount: number): string => `$${amount.toLocaleString('en-US')}`

export const view = (model: OnboardingModel): Html => {
  const h = html<Message>()

  return Ui.pageShell<Message>([
    h.header(
      [Ui.className<Message>('border-b border-[#222]')],
      [
        h.div(
          [
            Ui.className<Message>(
              'mx-auto flex min-h-12 w-[min(100%,72rem)] flex-wrap items-center justify-between gap-3 border-x border-[#222] px-4 py-2',
            ),
          ],
          [
            h.a(
              [
                h.Href(homeRouter()),
                Ui.className<Message>(
                  'text-xs font-bold uppercase tracking-[0.08em] text-white/60 no-underline hover:text-white/80',
                ),
              ],
              ['OpenAgents Autopilot'],
            ),
            h.a(
              [
                h.Href(githubLoginHref),
                Ui.className<Message>(
                  'inline-flex min-h-9 items-center border border-white/20 bg-white/10 px-3 text-xs font-semibold uppercase tracking-[0.08em] text-white/80 no-underline hover:bg-white/[0.04] hover:text-white',
                ),
              ],
              ['Log in with GitHub'],
            ),
          ],
        ),
      ],
    ),
    h.main(
      [
        Ui.className<Message>(
          'mx-auto grid min-h-[calc(100dvh-3rem)] w-[min(100%,72rem)] items-center border-x border-[#222] px-4 py-14 sm:px-6 lg:px-8',
        ),
      ],
      [model.step === 'github' ? landing() : fundingDemo(model)],
    ),
  ])
}

const landing = (): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('max-w-3xl')],
    [
      h.p(
        [
          Ui.className<Message>(
            'm-0 text-xs font-semibold uppercase tracking-[0.08em] text-white/45',
          ),
        ],
        ['OpenAgents Autopilot'],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 mt-7 max-w-[13ch] text-balance text-5xl font-semibold leading-none text-white/90 sm:text-6xl lg:text-7xl',
          ),
        ],
        ['Stop Babysitting Your AI'],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 mt-7 max-w-[46ch] text-base leading-7 text-white/60 sm:text-lg sm:leading-8',
          ),
        ],
        [
          'Launch coding agents. Close your laptop. Stay in the loop from anywhere.',
        ],
      ),
      h.div(
        [Ui.className<Message>('mt-7 grid gap-1 text-white/75')],
        [
          h.p(
            [Ui.className<Message>('m-0 text-lg font-semibold text-white/85')],
            ['Start work. Walk away.'],
          ),
          h.p(
            [Ui.className<Message>('m-0 text-sm text-white/50')],
            ['Your agents keep going.'],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('mt-9')],
        [
          h.a(
            [
              h.Href(githubLoginHref),
              Ui.className<Message>(
                'inline-grid min-h-11 place-items-center border border-[#f1efe8] bg-[#f1efe8] px-4 text-sm font-medium text-black no-underline hover:border-[#ffb400]',
              ),
            ],
            ['Log in with GitHub'],
          ),
        ],
      ),
    ],
  )
}

const fundingDemo = (model: OnboardingModel): Html => {
  const h = html<Message>()
  const bonus = bonusForAmount(model.fundingAmount)
  const total = model.fundingAmount + bonus

  return h.section(
    [
      Ui.className<Message>(
        'grid gap-5 lg:grid-cols-[minmax(0,1fr)_22rem] lg:items-start',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('max-w-2xl')],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 text-xs font-semibold uppercase tracking-[0.08em] text-white/45',
              ),
            ],
            ['OpenAgents Autopilot'],
          ),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 mt-7 max-w-[12ch] text-balance text-5xl font-semibold leading-none text-white/90 sm:text-6xl',
              ),
            ],
            ['Fund your account'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 mt-7 max-w-[42ch] text-base leading-7 text-white/60 sm:text-lg sm:leading-8',
              ),
            ],
            ['Start with $5. Larger balances include bonus credit.'],
          ),
          h.div(
            [Ui.className<Message>('mt-8 grid gap-4')],
            [
              h.label(
                [Ui.className<Message>('grid gap-4')],
                [
                  h.span(
                    [Ui.className<Message>(Ui.eyebrowClass)],
                    ['Funding amount'],
                  ),
                  h.input([
                    h.Type('range'),
                    h.Name('funding-amount'),
                    h.Min('5'),
                    h.Max('500'),
                    h.Step('5'),
                    h.Value(String(model.fundingAmount)),
                    h.AriaLabel('Funding amount'),
                    h.OnInput(value =>
                      UpdatedOnboardingFundingAmount({ value }),
                    ),
                    Ui.className<Message>(
                      'h-2 w-full accent-[#ffb400] focus-visible:outline-2 focus-visible:outline-[#ffb400]',
                    ),
                  ]),
                  h.div(
                    [
                      Ui.className<Message>(
                        'flex items-center justify-between text-xs text-white/40',
                      ),
                    ],
                    [h.span([], ['$5']), h.span([], ['$500'])],
                  ),
                ],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-px border border-[#222] bg-[#222] text-xs sm:grid-cols-3',
                  ),
                ],
                [
                  fact('pay', dollars(model.fundingAmount)),
                  fact('bonus', bonus === 0 ? '$0' : dollars(bonus)),
                  fact('credit', dollars(total)),
                ],
              ),
              h.div(
                [Ui.className<Message>('border border-[#222] bg-black p-4')],
                [
                  h.button(
                    [
                      h.Type('button'),
                      h.OnClick(ToggledOnboardingCoupon()),
                      Ui.className<Message>(
                        'text-xs font-semibold uppercase tracking-[0.08em] text-white/70 underline underline-offset-4 hover:text-[#ffb400]',
                      ),
                    ],
                    ['I have a coupon code'],
                  ),
                  model.isCouponOpen
                    ? h.label(
                        [Ui.className<Message>('mt-4 grid gap-1.5')],
                        [
                          h.span(
                            [Ui.className<Message>(Ui.eyebrowClass)],
                            ['Coupon code'],
                          ),
                          h.input([
                            h.Type('text'),
                            h.Name('coupon-code'),
                            h.Value(model.couponCode),
                            h.Placeholder('OPENAGENTS-10'),
                            h.OnInput(value =>
                              UpdatedOnboardingCouponCode({ value }),
                            ),
                            Ui.className<Message>(
                              `${Ui.inputClass} max-sm:text-base`,
                            ),
                          ]),
                        ],
                      )
                    : h.empty,
                ],
              ),
            ],
          ),
        ],
      ),
      Ui.orderSummary<Message>({
        title: 'Credit order',
        lines: [
          {
            label: 'You pay',
            value: dollars(model.fundingAmount),
          },
          {
            label: 'Bonus credit',
            value: bonus === 0 ? '$0' : dollars(bonus),
            tone: bonus > 0 ? 'positive' : 'neutral',
          },
          {
            label: 'Coupon',
            value:
              model.couponCode.trim() === '' ? 'none' : model.couponCode.trim(),
            tone: model.couponCode.trim() === '' ? 'neutral' : 'info',
          },
          {
            label: 'Account credit',
            value: dollars(total),
            strong: true,
          },
        ],
        action: Ui.linkButton<Message>({
          href: githubLoginHref,
          label: 'Continue with GitHub',
        }),
      }),
    ],
  )
}

const fact = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid min-h-20 gap-1 bg-black p-3')],
    [
      h.div([Ui.className<Message>('text-white/35')], [label]),
      h.div([Ui.className<Message>('break-words text-white/75')], [value]),
    ],
  )
}
