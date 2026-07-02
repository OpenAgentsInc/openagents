import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  DefaultKhalaFleetDelegationParameterSet,
  KhalaFleetDelegationAdmittedParametersEnv,
  KhalaFleetDelegationParameterSet,
  KhalaFleetDelegationParameterSetSchemaVersion,
  KhalaFleetDelegateModuleError,
  khalaFleetDelegationDispatchAttempts,
  khalaFleetDelegationParametersFromEnv,
  prepareKhalaFleetDelegateWork,
  renderKhalaFleetDelegationObjective,
  renderDefaultKhalaFleetDelegationObjective,
  resolveKhalaFleetDelegateWorkerKind,
  runKhalaFleetDelegateProgram,
  selectKhalaFleetDelegateAccount,
  type KhalaFleetDelegateAccount,
  type KhalaFleetDelegateAdvertiseResult,
  type KhalaFleetDelegateBlockerCode,
  type KhalaFleetDelegateDispatchResult,
  type KhalaFleetDelegateModuleName,
  type KhalaFleetDelegateModules,
  type KhalaFleetDelegateProgramResult,
} from "./fleet-delegate-program.js"

const readyAccount = (overrides: Partial<KhalaFleetDelegateAccount> = {}): KhalaFleetDelegateAccount => ({
  accountRef: "codex-2",
  availableSlots: 1,
  readiness: "ready",
  ...overrides,
})

const advertised = (
  available: number,
  accounts: ReadonlyArray<KhalaFleetDelegateAccount> = [readyAccount({ availableSlots: available })],
): KhalaFleetDelegateAdvertiseResult => ({
  capacity: {
    accounts,
    available,
    max: Math.max(available, 1),
  },
  heartbeatRef: `heartbeat.capacity.${available}`,
})

const completedModules = (
  overrides: Partial<KhalaFleetDelegateModules> = {},
): KhalaFleetDelegateModules => ({
  advertiseCapacity: () => Effect.succeed(advertised(1)),
  dispatch: () =>
    Effect.succeed({
      assignmentRef: "assignment.public.khala_fleet_delegate.test",
      ok: true,
    }),
  ensurePylon: () => Effect.succeed({ pylonRef: "pylon.local.test" }),
  verifyCloseout: () => Effect.succeed({ ok: true }),
  ...overrides,
})

const admittedParameters = (
  overrides: Partial<KhalaFleetDelegationParameterSet> = {},
): KhalaFleetDelegationParameterSet =>
  new KhalaFleetDelegationParameterSet({
    actionSubmissionRef: "action_submission.khala_fleet_delegation.test",
    candidateRef: "candidate.khala_fleet_delegation.test",
    parameterSetRef: "parameter_set.khala_fleet_delegation.test.v1",
    schemaVersion: KhalaFleetDelegationParameterSetSchemaVersion,
    source: "admitted_candidate",
    ...overrides,
  })

type AdverseMatrixCase = Readonly<{
  expectedBlockerCode?: KhalaFleetDelegateBlockerCode
  expectedStatus: KhalaFleetDelegateProgramResult["status"]
  expectedFallbackModule?: KhalaFleetDelegateModuleName
  modules: () => KhalaFleetDelegateModules
  name: string
}>

const adverseMatrixCases: readonly AdverseMatrixCase[] = [
  {
    expectedFallbackModule: "advertise_capacity",
    expectedStatus: "completed",
    modules: () => {
      let advertiseCalls = 0
      let dispatchCalls = 0
      return completedModules({
        advertiseCapacity: input => {
          advertiseCalls += 1
          return Effect.succeed(advertiseCalls === 1
            ? {
                capacity: {
                  accounts: [readyAccount({ availableSlots: undefined })],
                  available: 0,
                  max: 1,
                },
                heartbeatRef: `heartbeat.${input.reason}.zero_one`,
              }
            : advertised(1, [readyAccount({ availableSlots: 1 })]))
        },
        dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
          dispatchCalls += 1
          return Effect.succeed(dispatchCalls === 1
            ? {
                blockerCode: "no_available_codex_capacity",
                message: "capacity unavailable after the first dispatch probe",
                ok: false,
                refs: ["blocker.public.pylon_dispatch.no_available_codex_capacity"],
              }
            : {
                assignmentRef: "assignment.public.khala_fleet_delegate.matrix.capacity",
                ok: true,
              })
        },
      })
    },
    name: "capacity 0/1 refreshes through advertise_capacity",
  },
  {
    expectedFallbackModule: "advertise_capacity",
    expectedStatus: "completed",
    modules: () => {
      let dispatchCalls = 0
      return completedModules({
        advertiseCapacity: input =>
          Effect.succeed({
            ...advertised(1),
            heartbeatRef: `heartbeat.${input.reason}`,
          }),
        dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
          dispatchCalls += 1
          return Effect.succeed(dispatchCalls === 1
            ? {
                blockerCode: "stale_heartbeat",
                message: "presence heartbeat is stale",
                ok: false,
                refs: ["blocker.public.pylon_dispatch.stale_heartbeat"],
              }
            : {
                assignmentRef: "assignment.public.khala_fleet_delegate.matrix.stale",
                ok: true,
              })
        },
      })
    },
    name: "stale heartbeat 409 refreshes capacity and retries",
  },
  {
    expectedFallbackModule: "dispatch",
    expectedStatus: "completed",
    modules: () => {
      let backoffs = 0
      let dispatchCalls = 0
      return completedModules({
        backoff: () => {
          backoffs += 1
          return Effect.void
        },
        dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
          dispatchCalls += 1
          return Effect.succeed(dispatchCalls === 1
            ? {
                blockerCode: "duplicate_active_assignment",
                message: "duplicate active assignment",
                ok: false,
                refs: ["blocker.public.pylon_dispatch.duplicate_active_assignment"],
              }
            : {
                assignmentRef: `assignment.public.khala_fleet_delegate.matrix.duplicate.${backoffs}`,
                ok: true,
              })
        },
      })
    },
    name: "duplicate active assignment backs off and retries",
  },
  {
    expectedBlockerCode: "credentials_missing",
    expectedStatus: "blocked",
    modules: () => completedModules({
      advertiseCapacity: () =>
        Effect.succeed(advertised(1, [
          readyAccount({
            accountRef: "codex-2",
            availableSlots: 1,
            readiness: "credentials_missing",
          }),
        ])),
    }),
    name: "credentials-missing account returns typed actionable blocker",
  },
  {
    expectedBlockerCode: "revoked",
    expectedStatus: "blocked",
    modules: () => completedModules({
      advertiseCapacity: () =>
        Effect.succeed(advertised(1, [
          readyAccount({
            accountRef: "codex-2",
            availableSlots: 1,
            readiness: "revoked",
          }),
        ])),
    }),
    name: "revoked account returns typed actionable blocker",
  },
  {
    expectedBlockerCode: "load_gated",
    expectedStatus: "blocked",
    modules: () => completedModules({
      advertiseCapacity: () =>
        Effect.succeed({
          capacity: {
            ...advertised(1).capacity,
            loadGated: true,
          },
          heartbeatRef: "heartbeat.load_gated",
        }),
      dispatch: () =>
        Effect.fail(new KhalaFleetDelegateModuleError({
          blockerCode: "dispatch_failed",
          message: "dispatch should not run while load-gated",
          module: "dispatch",
          refs: ["blocker.public.khala_fleet_delegate.dispatch_failed"],
        })),
    }),
    name: "high machine load returns load_gated before dispatch",
  },
]

const resultContainsLegacyBareCapacityDeadEnd = (
  result: KhalaFleetDelegateProgramResult,
): boolean =>
  JSON.stringify(result).includes("codex_spawn_failed: No Pylon Codex assignment capacity is available right now")

describe("khala.fleet.delegate deterministic program", () => {
  describe("admitted delegation parameters", () => {
    test("defaults are safe when no candidate is admitted", () => {
      expect(khalaFleetDelegationParametersFromEnv({})).toEqual(DefaultKhalaFleetDelegationParameterSet)
      expect(khalaFleetDelegationDispatchAttempts(DefaultKhalaFleetDelegationParameterSet)).toBe(4)
      expect(renderKhalaFleetDelegationObjective({
        objective: "Implement public issue #7736.",
        repo: "OpenAgentsInc/openagents",
        verify: "bun test",
      })).toBe("Implement public issue #7736.")
    })

    test("switching account ranking changes selection and reverting restores the default", () => {
      const accounts = [
        readyAccount({ accountRef: "(default)", availableSlots: 3, isDefault: true }),
        readyAccount({ accountRef: "codex-2", availableSlots: 3 }),
      ]

      expect(selectKhalaFleetDelegateAccount({}, accounts)).toMatchObject({
        account: { accountRef: "codex-2" },
        status: "selected",
      })
      expect(selectKhalaFleetDelegateAccount({}, accounts, admittedParameters({
        accountRanking: { heuristic: "default_ready_highest_slots" },
      }))).toMatchObject({
        account: { accountRef: "(default)" },
        status: "selected",
      })
      expect(selectKhalaFleetDelegateAccount({}, accounts, DefaultKhalaFleetDelegationParameterSet)).toMatchObject({
        account: { accountRef: "codex-2" },
        status: "selected",
      })
    })

    test("auto workerKind picks claude when codex has no free slot and claude does", () => {
      const parameters = admittedParameters({
        delegationTarget: { workerKind: "auto" },
      })
      const accounts = [
        readyAccount({ accountRef: "codex", availableSlots: 0, workerKind: "codex" }),
        readyAccount({ accountRef: "claude", availableSlots: 1, workerKind: "claude" }),
      ]

      expect(resolveKhalaFleetDelegateWorkerKind(accounts, parameters)).toBe("claude")
      expect(selectKhalaFleetDelegateAccount({}, accounts, parameters)).toMatchObject({
        account: { accountRef: "claude" },
        status: "selected",
        workerKind: "claude",
      })
    })

    test("auto workerKind picks codex when advertised free slots tie", () => {
      const parameters = admittedParameters({
        delegationTarget: { workerKind: "auto" },
      })
      const accounts = [
        readyAccount({ accountRef: "claude", availableSlots: 1, workerKind: "claude" }),
        readyAccount({ accountRef: "codex", availableSlots: 1, workerKind: "codex" }),
      ]

      expect(resolveKhalaFleetDelegateWorkerKind(accounts, parameters)).toBe("codex")
      expect(selectKhalaFleetDelegateAccount({}, accounts, parameters)).toMatchObject({
        account: { accountRef: "codex" },
        status: "selected",
        workerKind: "codex",
      })
    })

    test("capacity blocker vocabulary is keyed by explicit worker kind", () => {
      const selected = selectKhalaFleetDelegateAccount({}, [
        readyAccount({ accountRef: "claude", availableSlots: 0, workerKind: "claude" }),
      ], admittedParameters({
        delegationTarget: { workerKind: "claude" },
      }))

      expect(selected).toMatchObject({
        blockerCode: "no_available_claude_capacity",
        blockerRefs: ["blocker.public.pylon_dispatch.no_available_claude_capacity"],
        status: "blocked",
      })
    })

    test("switching retry budget changes duplicate-assignment recovery behavior", async () => {
      let oneAttemptDispatchCalls = 0
      const oneAttempt = await Effect.runPromise(runKhalaFleetDelegateProgram({
        objective: "Run fixture.",
      }, completedModules({
        dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
          oneAttemptDispatchCalls += 1
          return Effect.succeed({
            blockerCode: "duplicate_active_assignment",
            message: "duplicate",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.duplicate_active_assignment"],
          })
        },
      }), {
        parameters: admittedParameters({
          retryBackoff: { dispatchAttempts: 1 },
        }),
      }))

      expect(oneAttempt.status).toBe("blocked")
      expect(oneAttemptDispatchCalls).toBe(1)

      let defaultDispatchCalls = 0
      const reverted = await Effect.runPromise(runKhalaFleetDelegateProgram({
        objective: "Run fixture.",
      }, completedModules({
        dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
          defaultDispatchCalls += 1
          return Effect.succeed(defaultDispatchCalls === 1
            ? {
                blockerCode: "duplicate_active_assignment",
                message: "duplicate",
                ok: false,
                refs: ["blocker.public.pylon_dispatch.duplicate_active_assignment"],
              }
            : {
                assignmentRef: "assignment.public.khala_fleet_delegate.reverted_retry",
                ok: true,
              })
        },
      }), {
        parameters: DefaultKhalaFleetDelegationParameterSet,
      }))

      expect(reverted.status).toBe("completed")
      expect(defaultDispatchCalls).toBe(2)
    })

    test("objective template and verify criteria are read from the admitted set", () => {
      const parameters = admittedParameters({
        objectiveTemplate:
          "Optimized issue {issue}: {objective} Repo={repo}. Verify with {verify}.",
        verifyCriteria: { defaultVerify: "bun test packages/khala-tools" },
      })

      expect(renderKhalaFleetDelegationObjective({
        issue: 7736,
        objective: "Wire admitted parameters.",
        repo: "OpenAgentsInc/openagents",
      }, parameters)).toBe(
        "Optimized issue 7736: Wire admitted parameters. Repo=OpenAgentsInc/openagents. Verify with bun test packages/khala-tools.",
      )
      expect(prepareKhalaFleetDelegateWork({
        claimRef: "claim.public.t4_2.test",
        commit: "0123456789abcdef0123456789abcdef01234567",
        objective: "Wire admitted parameters.",
        repo: "OpenAgentsInc/openagents",
      }, parameters)).toMatchObject({
        kind: "repo",
        verify: "bun test packages/khala-tools",
      })
    })

    test("objective template does not duplicate an already-rendered discipline block", () => {
      const parameters = admittedParameters({
        objectiveTemplate: [
          "{objective}",
          "",
          "Public issue: #{issue}.",
          "Claim: {claimRef}.",
          "Repository: {repo}.",
          "Base branch: {branch} at {commit}.",
          "Verification command ref: {verify}.",
        ].join("\n"),
      })

      const prompt = renderKhalaFleetDelegationObjective({
        branch: "main",
        claimRef: "claim.public.t4_2.dedupe",
        commit: "0123456789abcdef0123456789abcdef01234567",
        issue: 7835,
        objective: "Implement T4.2 prompt discipline.",
        repo: "OpenAgentsInc/openagents",
        verify: "command.public.pylon_khala.verify.d32c71ee8e1025e99460d008",
      }, parameters)

      expect(prompt.match(/Claim: claim\.public\.t4_2\.dedupe\./gu)).toHaveLength(1)
      expect(prompt.match(/Verification command ref:/gu)).toHaveLength(1)
    })

    test("env admission decodes a bounded parameter set and rejects unsafe text", () => {
      const parameters = admittedParameters({
        advertiseCapacity: { perAccountConcurrency: 4 },
        objectiveTemplate: "Admitted: {objective}",
      })
      const decoded = khalaFleetDelegationParametersFromEnv({
        [KhalaFleetDelegationAdmittedParametersEnv]: JSON.stringify(parameters),
      })

      expect(decoded.parameterSetRef).toBe(parameters.parameterSetRef)
      expect(decoded.advertiseCapacity?.perAccountConcurrency).toBe(4)
      expect(() =>
        khalaFleetDelegationParametersFromEnv({
          [KhalaFleetDelegationAdmittedParametersEnv]: JSON.stringify(admittedParameters({
            objectiveTemplate: "Read /Users/example/auth.json before dispatch.",
          })),
        }),
      ).toThrow(/public-safe/)
    })
  })

  describe("adverse-condition recovery matrix", () => {
    for (const matrixCase of adverseMatrixCases) {
      test(matrixCase.name, async () => {
        const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
          objective: "Run the public fixture.",
        }, matrixCase.modules()))

        expect(result.signature).toBe("khala.fleet.delegate")
        expect(result.status).toBe(matrixCase.expectedStatus)
        expect(result.trace.length).toBeGreaterThan(0)
        expect(resultContainsLegacyBareCapacityDeadEnd(result)).toBe(false)

        if (result.status === "blocked") {
          if (matrixCase.expectedBlockerCode !== undefined) {
            expect(result.blockerCode).toBe(matrixCase.expectedBlockerCode)
          }
          expect(result.blockerRefs.length).toBeGreaterThan(0)
          expect(result.blockerRefs.every(ref => ref.startsWith("blocker."))).toBe(true)
          expect(result.message.trim().length).toBeGreaterThan(0)
        } else {
          expect(result.assignmentRef).toMatch(/^assignment\.public\.khala_fleet_delegate\.matrix\./)
        }

        if (matrixCase.expectedFallbackModule !== undefined) {
          expect(result.trace.map(step => step.fallbackModule).filter(Boolean))
            .toContain(matrixCase.expectedFallbackModule)
        }
      })
    }
  })

  test("reaches dispatch from a cold 0/1 start by advertising capacity first", async () => {
    const calls: string[] = []
    const modules = completedModules({
      advertiseCapacity: input => {
        calls.push(`advertise:${input.reason}`)
        return Effect.succeed(advertised(1, [readyAccount({ availableSlots: 1 })]))
      },
      dispatch: input => {
        calls.push(`dispatch:${input.account.accountRef}:${input.attempt}`)
        return Effect.succeed({
          assignmentRef: "assignment.public.khala_fleet_delegate.cold_start",
          ok: true,
        })
      },
      ensurePylon: () => {
        calls.push("ensure")
        return Effect.succeed({ pylonRef: "pylon.local.test", started: true })
      },
    })

    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run the public fixture.",
    }, modules))

    expect(result.status).toBe("completed")
    expect(calls).toEqual(["ensure", "advertise:initial", "dispatch:codex-2:1"])
    expect(result.trace.map(step => step.module)).toEqual([
      "ensure_pylon",
      "advertise_capacity",
      "select_account",
      "prepare_work",
      "dispatch",
      "verify_closeout",
    ])
    expect(result.trace[0]?.status).toBe("recovered")
    expect(result.trace[3]?.status).toBe("recovered")
  })

  test("select_account skips missing and revoked accounts before choosing a ready one", () => {
    const selected = selectKhalaFleetDelegateAccount({}, [
      readyAccount({ accountRef: "codex", readiness: "credentials_missing" }),
      readyAccount({ accountRef: "codex-3", readiness: "revoked" }),
      readyAccount({ accountRef: "codex-2", readiness: "ready" }),
    ])

    expect(selected).toMatchObject({
      account: { accountRef: "codex-2" },
      status: "selected",
    })
  })

  test("select_account returns a typed blocker when a requested account is revoked", () => {
    const selected = selectKhalaFleetDelegateAccount({ accountRef: "codex-3" }, [
      readyAccount({ accountRef: "codex-3", readiness: "revoked" }),
      readyAccount({ accountRef: "codex-2", readiness: "ready" }),
    ])

    expect(selected).toMatchObject({
      blockerCode: "revoked",
      blockerRefs: ["blocker.public.khala_fleet_delegate.revoked"],
      status: "blocked",
    })
  })

  test("prepare_work falls back to the fixture when no pins are provided", () => {
    expect(prepareKhalaFleetDelegateWork({ objective: "Run fixture." })).toEqual({
      fixture: true,
      kind: "fixture",
    })
  })

  test("prepare_work rejects partial real-work pins", () => {
    expect(() =>
      prepareKhalaFleetDelegateWork({
        objective: "Run real work.",
        repo: "OpenAgentsInc/openagents",
      }),
    ).toThrow("missing commit, verify, claimRef")
  })

  test("prepare_work requires claimRef for real work and preserves issue metadata", () => {
    expect(prepareKhalaFleetDelegateWork({
      claimRef: "claim.public.t4_2.issue_7835",
      commit: "0123456789abcdef0123456789abcdef01234567",
      issue: 7835,
      objective: "Implement public issue #7835.",
      repo: "OpenAgentsInc/openagents",
      verify: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
    })).toEqual({
      branch: "main",
      claimRef: "claim.public.t4_2.issue_7835",
      commit: "0123456789abcdef0123456789abcdef01234567",
      issue: 7835,
      kind: "repo",
      repo: "OpenAgentsInc/openagents",
      verify: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
    })
  })

  test("default real-work prompt cites issue, claim, verification ref, and PR convention", () => {
    const prompt = renderDefaultKhalaFleetDelegationObjective({
      branch: "main",
      claimRef: "claim.public.t4_2.issue_7835",
      commit: "0123456789abcdef0123456789abcdef01234567",
      issue: 7835,
      objective: "Implement T4.2 prompt/pin discipline.",
      repo: "OpenAgentsInc/openagents",
      verify: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
    })

    expect(prompt).toContain("Public issue: #7835.")
    expect(prompt).toContain("Claim: claim.public.t4_2.issue_7835.")
    expect(prompt).toContain("Verification command ref: command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2.")
    expect(prompt).toContain('include "Closes #7835" in the PR body')
    expect(prompt).toContain("ready non-draft PR")
    expect(prompt).toContain("do not merge it")
  })

  test("dispatch refreshes stale heartbeat capacity and retries once", async () => {
    let dispatchCalls = 0
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      advertiseCapacity: input =>
        Effect.succeed({
          ...advertised(1),
          heartbeatRef: `heartbeat.${input.reason}`,
        }),
      dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
        dispatchCalls += 1
        return Effect.succeed(dispatchCalls === 1
          ? {
            blockerCode: "stale_heartbeat",
            message: "heartbeat stale",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.stale_heartbeat"],
          }
          : {
            assignmentRef: "assignment.public.khala_fleet_delegate.retry",
            ok: true,
          })
      },
    })))

    expect(result.status).toBe("completed")
    expect(dispatchCalls).toBe(2)
    expect(result.trace.map(step => `${step.module}:${step.status}`)).toContain("advertise_capacity:recovered")
    expect(result.trace.map(step => step.fallbackModule).filter(Boolean)).toContain("advertise_capacity")
  })

  test("dispatch backs off duplicate_active_assignment before retrying", async () => {
    let backoffs = 0
    let dispatchCalls = 0
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      backoff: () => {
        backoffs += 1
        return Effect.void
      },
      dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
        dispatchCalls += 1
        return Effect.succeed(dispatchCalls === 1
          ? {
            blockerCode: "duplicate_active_assignment",
            message: "duplicate",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.duplicate_active_assignment"],
          }
          : {
            assignmentRef: "assignment.public.khala_fleet_delegate.duplicate_retry",
            ok: true,
          })
      },
    })))

    expect(result.status).toBe("completed")
    expect(backoffs).toBe(1)
    expect(dispatchCalls).toBe(2)
    expect(result.trace.map(step => step.fallbackModule).filter(Boolean)).toContain("dispatch")
  })

  test("dispatch loops no_available_codex_capacity back through advertise_capacity", async () => {
    let advertiseCalls = 0
    let dispatchCalls = 0
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      advertiseCapacity: input => {
        advertiseCalls += 1
        return Effect.succeed({
          ...advertised(1),
          heartbeatRef: `heartbeat.${advertiseCalls}.${input.reason}`,
        })
      },
      dispatch: (): Effect.Effect<KhalaFleetDelegateDispatchResult> => {
        dispatchCalls += 1
        return Effect.succeed(dispatchCalls === 1
          ? {
            blockerCode: "no_available_codex_capacity",
            message: "capacity unavailable",
            ok: false,
            refs: ["blocker.public.pylon_dispatch.no_available_codex_capacity"],
          }
          : {
            assignmentRef: "assignment.public.khala_fleet_delegate.capacity_retry",
            ok: true,
          })
      },
    })))

    expect(result.status).toBe("completed")
    expect(advertiseCalls).toBe(2)
    expect(result.trace.map(step => step.fallbackModule).filter(Boolean)).toContain("advertise_capacity")
  })

  test("verify_closeout returns verify_failed as a typed blocker", async () => {
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      verifyCloseout: () =>
        Effect.succeed({
          blockerRefs: ["blocker.public.khala_fleet_delegate.verify_failed"],
          message: "token rows missing",
          ok: false,
        }),
    })))

    expect(result).toMatchObject({
      blockerCode: "verify_failed",
      blockerRefs: ["blocker.public.khala_fleet_delegate.verify_failed"],
      status: "blocked",
    })
    expect(result.trace.at(-1)).toMatchObject({
      module: "verify_closeout",
      precondition: "closeout_verified",
      status: "blocked",
    })
  })

  test("module failures surface through the typed taxonomy", async () => {
    const result = await Effect.runPromise(runKhalaFleetDelegateProgram({
      objective: "Run fixture.",
    }, completedModules({
      ensurePylon: () =>
        Effect.fail(new KhalaFleetDelegateModuleError({
          blockerCode: "pylon_unavailable",
          message: "Pylon is offline.",
          module: "ensure_pylon",
          refs: ["blocker.public.khala_fleet_delegate.pylon_unavailable"],
        })),
    })))

    expect(result).toMatchObject({
      blockerCode: "pylon_unavailable",
      blockerRefs: ["blocker.public.khala_fleet_delegate.pylon_unavailable"],
      status: "blocked",
    })
  })
})
