import type { TassadarAlmNumericModel } from "./numeric-executor.js"

export type TassadarProgramInstruction = Readonly<Record<string, unknown>>

export type TassadarProgramPayload = Readonly<{
  program_id: string
  profile_id: string
  local_count: number
  memory_slots: number
  initial_memory?: ReadonlyArray<number>
  instructions: ReadonlyArray<TassadarProgramInstruction>
}>

export type TassadarCompiledProgramFixture = Readonly<{
  fixtureId: string
  programId: string
  programDigest: string
  workloadKind: string
  profileId: string
  program: TassadarProgramPayload
  model: TassadarAlmNumericModel
  steps: ReadonlyArray<ReadonlyArray<number>>
  expectedTraceDigest: string
  expectedModelDigest: string
  expectedFinalRow: ReadonlyArray<number> | null
  expectedOutputs: ReadonlyArray<number>
  halted: boolean
  compileReceiptRefs: ReadonlyArray<string>
}>

export type TassadarCompiledProgramCorpus = Readonly<{
  schemaVersion: number
  corpusId: string
  generatedBy: string
  claimBoundary: string
  programCount: number
  fixtures: ReadonlyArray<TassadarCompiledProgramFixture>
  corpusDigest: string
}>

export const tassadarCompiledProgramCorpus: TassadarCompiledProgramCorpus = {
  "schemaVersion": 1,
  "corpusId": "tassadar_alm.numeric_program_corpus.v1",
  "generatedBy": "psionic crates/psionic-compiler tassadar_alm_numeric_program_corpus_v1",
  "claimBoundary": "the numeric model is a faithful f64 re-encoding of one compiled ALM bundle - explicit coefficient arrays executed with hard-max attention inside a checked exactness window of 2^53 - not a trained transformer; it claims integer parity only while every intermediate stays inside the window, refuses when one does not, and makes no softmax, learning, or served-route claim",
  "programCount": 4,
  "fixtures": [
    {
      "fixtureId": "tassadar_corpus.loop_sum_v1.numeric_fixture.v1",
      "programId": "tassadar_corpus.loop_sum_v1",
      "programDigest": "69815f8c4340fc9d94ebfbfa8ef989450910c7669fcd85f4c7ffee4bccffdb6c",
      "workloadKind": "control_flow.backward_branch_sum",
      "profileId": "tassadar.wasm.article_i32_compute.v1",
      "program": {
        "program_id": "tassadar_corpus.loop_sum_v1",
        "profile_id": "tassadar.wasm.article_i32_compute.v1",
        "local_count": 2,
        "memory_slots": 1,
        "initial_memory": [
          0
        ],
        "instructions": [
          {
            "opcode": "i32_const",
            "value": 0
          },
          {
            "opcode": "local_set",
            "local": 0
          },
          {
            "opcode": "i32_const",
            "value": 1
          },
          {
            "opcode": "local_set",
            "local": 1
          },
          {
            "opcode": "local_get",
            "local": 0
          },
          {
            "opcode": "local_get",
            "local": 1
          },
          {
            "opcode": "i32_add"
          },
          {
            "opcode": "local_set",
            "local": 0
          },
          {
            "opcode": "local_get",
            "local": 1
          },
          {
            "opcode": "i32_const",
            "value": 1
          },
          {
            "opcode": "i32_add"
          },
          {
            "opcode": "local_set",
            "local": 1
          },
          {
            "opcode": "local_get",
            "local": 1
          },
          {
            "opcode": "i32_const",
            "value": 6
          },
          {
            "opcode": "i32_lt"
          },
          {
            "opcode": "br_if",
            "target_pc": 4
          },
          {
            "opcode": "local_get",
            "local": 0
          },
          {
            "opcode": "output"
          },
          {
            "opcode": "return"
          }
        ]
      },
      "model": {
        "schema_version": 1,
        "model_id": "alm.numeric.tassadar.alm_wasm_interpreter.v1.tassadar_corpus.loop_sum_v1",
        "graph_digest": "30b042c3e3880c3b13b3df9d5388da334f982c44a295f03f7a4448f54a6da223",
        "bundle_digest": "3c34af4e07a17713b597306024c10390b1c78340eed89f43163d89898f52bff9",
        "input_field_count": 1,
        "slot_count": 60,
        "layer_count": 6,
        "seed_writes": [
          [
            0,
            0,
            0
          ],
          [
            0,
            1,
            0
          ],
          [
            0,
            2,
            2
          ],
          [
            0,
            3,
            0
          ],
          [
            0,
            4,
            0
          ],
          [
            0,
            5,
            1
          ],
          [
            0,
            6,
            2
          ],
          [
            0,
            7,
            1
          ],
          [
            0,
            8,
            1
          ],
          [
            0,
            9,
            0
          ],
          [
            0,
            10,
            1
          ],
          [
            0,
            11,
            1
          ],
          [
            0,
            12,
            3
          ],
          [
            0,
            13,
            0
          ],
          [
            0,
            14,
            2
          ],
          [
            0,
            15,
            0
          ],
          [
            0,
            16,
            1
          ],
          [
            0,
            17,
            1
          ],
          [
            0,
            18,
            0
          ],
          [
            0,
            19,
            1
          ],
          [
            0,
            20,
            3
          ],
          [
            0,
            21,
            0
          ],
          [
            0,
            22,
            2
          ],
          [
            0,
            23,
            1
          ],
          [
            0,
            24,
            1
          ],
          [
            0,
            25,
            1
          ],
          [
            0,
            26,
            0
          ],
          [
            0,
            27,
            6
          ],
          [
            0,
            28,
            6
          ],
          [
            0,
            29,
            0
          ],
          [
            0,
            30,
            9
          ],
          [
            0,
            31,
            4
          ],
          [
            0,
            32,
            1
          ],
          [
            0,
            33,
            0
          ],
          [
            0,
            34,
            10
          ],
          [
            0,
            35,
            0
          ],
          [
            0,
            36,
            11
          ],
          [
            0,
            37,
            0
          ],
          [
            0,
            38,
            11
          ],
          [
            0,
            39,
            0
          ],
          [
            0,
            40,
            11
          ],
          [
            0,
            41,
            0
          ],
          [
            1,
            -1,
            0
          ],
          [
            1,
            0,
            0
          ],
          [
            2,
            0,
            0
          ],
          [
            2,
            1,
            0
          ],
          [
            3,
            0,
            0
          ],
          [
            4,
            0,
            0
          ],
          [
            4,
            1,
            0
          ],
          [
            4,
            2,
            0
          ]
        ],
        "wiring": [
          {
            "out_slot": 0,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 1,
            "bias": 0,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 2,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 3,
            "bias": 2,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 10,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 11,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 14,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 16,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 17,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 18,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 19,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 20,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 21,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 22,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 23,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -11,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -1,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ],
              [
                -1,
                37
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                38
              ],
              [
                -1,
                39
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                48
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                51
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ],
              [
                -1,
                53
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": -1,
            "terms": [
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ],
              [
                -1,
                59
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 1,
            "terms": [
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                40
              ],
              [
                -1,
                41
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 21,
            "bias": 0,
            "terms": [
              [
                1,
                42
              ],
              [
                -1,
                43
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                44
              ],
              [
                -1,
                45
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 23,
            "bias": 0,
            "terms": [
              [
                1,
                46
              ],
              [
                -1,
                47
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                54
              ],
              [
                -1,
                55
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 28,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                16
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 8,
            "bias": 1,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 28,
            "bias": 1,
            "terms": [
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 29,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                7
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                20
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                20
              ],
              [
                -1,
                21
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                21
              ],
              [
                -1,
                22
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                22
              ],
              [
                -1,
                23
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                23
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                24
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                18
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                29
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                1,
                26
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 19,
            "bias": -1,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                -1,
                25
              ],
              [
                -1,
                26
              ],
              [
                -1,
                27
              ],
              [
                -1,
                35
              ],
              [
                -1,
                30
              ],
              [
                -1,
                31
              ],
              [
                -1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 21,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                1,
                30
              ],
              [
                1,
                31
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                -1,
                27
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 20,
            "bias": 1,
            "terms": [
              [
                1,
                37
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                17
              ],
              [
                1,
                8
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                1,
                18
              ],
              [
                1,
                21
              ],
              [
                1,
                22
              ],
              [
                1,
                23
              ],
              [
                1,
                24
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 10,
            "bias": -1000,
            "terms": [
              [
                1,
                26
              ],
              [
                1000,
                38
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 11,
            "bias": -1000,
            "terms": [
              [
                1,
                27
              ],
              [
                1000,
                15
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 12,
            "bias": -1000,
            "terms": [
              [
                1,
                35
              ],
              [
                1000,
                16
              ]
            ],
            "input_field": null,
            "phase": 24
          }
        ],
        "attention": [
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 1,
              "out_slot": 5,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 4,
              "out_slot": 6,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 7,
              "out_slot": 9,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 17,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 14,
              "out_slot": 18,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 8,
              "out_slot": 7,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 23,
              "out_slot": 0,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 24,
              "out_slot": 7,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 7,
            "out_slot": 9,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 35,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 36,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 38,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 39,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 17,
            "out_slot": 42,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 18,
            "out_slot": 43,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 44,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 20,
            "out_slot": 45,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 21,
            "out_slot": 46,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 22,
            "out_slot": 47,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 23,
            "out_slot": 48,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 49,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 50,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 51,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 52,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 53,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 54,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 55,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 56,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 18,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 15
          },
          {
            "value_slot": 28,
            "gate_slot": 27,
            "out_slot": 12,
            "phase": 15
          },
          {
            "value_slot": 25,
            "gate_slot": 14,
            "out_slot": 15,
            "phase": 15
          },
          {
            "value_slot": 26,
            "gate_slot": 14,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 7,
            "gate_slot": 10,
            "out_slot": 23,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 11,
            "out_slot": 24,
            "phase": 19
          },
          {
            "value_slot": 18,
            "gate_slot": 6,
            "out_slot": 25,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 26,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 27,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 36,
            "phase": 19
          },
          {
            "value_slot": 21,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 19
          },
          {
            "value_slot": 22,
            "gate_slot": 14,
            "out_slot": 38,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 28,
            "out_slot": 39,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 19
          },
          {
            "value_slot": 35,
            "gate_slot": 14,
            "out_slot": 42,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 4,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 23
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 34,
            "out_slot": 21,
            "phase": 23
          },
          {
            "value_slot": 10,
            "gate_slot": 30,
            "out_slot": 22,
            "phase": 23
          },
          {
            "value_slot": 11,
            "gate_slot": 31,
            "out_slot": 23,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 32,
            "out_slot": 24,
            "phase": 23
          },
          {
            "value_slot": 13,
            "gate_slot": 33,
            "out_slot": 25,
            "phase": 23
          },
          {
            "value_slot": 19,
            "gate_slot": 38,
            "out_slot": 26,
            "phase": 23
          },
          {
            "value_slot": 40,
            "gate_slot": 15,
            "out_slot": 27,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 15,
            "out_slot": 28,
            "phase": 23
          },
          {
            "value_slot": 41,
            "gate_slot": 16,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 16,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 42,
            "out_slot": 37,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 10,
            "value_slot": 7
          },
          {
            "channel": 2,
            "key_slot": 11,
            "value_slot": 28
          },
          {
            "channel": 3,
            "key_slot": 12,
            "value_slot": 36
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 0
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 19
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 6
          }
        ],
        "output_slots": [
          42,
          37,
          0,
          19,
          6
        ]
      },
      "steps": [
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ]
      ],
      "expectedTraceDigest": "2465d2c2af5077b4cf44c6eddbdc5aba2859029e30062f49a30e669acfc8e9d2",
      "expectedModelDigest": "855c94b4f2c46d8c0e0b28993368d71fa82d3738cc3535deaff32e0ccc075381",
      "expectedFinalRow": [
        0,
        0,
        19,
        0,
        1
      ],
      "expectedOutputs": [
        15
      ],
      "halted": true,
      "compileReceiptRefs": [
        "receipt.psionic.tassadar_program.69815f8c4340fc9d",
        "receipt.psionic.tassadar_graph.30b042c3e3880c3b",
        "receipt.psionic.tassadar_bundle.3c34af4e07a17713",
        "receipt.psionic.tassadar_numeric_model.855c94b4f2c46d8c",
        "receipt.psionic.tassadar_trace.2465d2c2af5077b4"
      ]
    },
    {
      "fixtureId": "tassadar_corpus.mul_add_v1.numeric_fixture.v1",
      "programId": "tassadar_corpus.mul_add_v1",
      "programDigest": "3a90befea6efb5f7d7801034e69c93bb970ab2f7563a31821444b967ecaf51c7",
      "workloadKind": "arithmetic.mul_add",
      "profileId": "tassadar.wasm.core_i32.v2",
      "program": {
        "program_id": "tassadar_corpus.mul_add_v1",
        "profile_id": "tassadar.wasm.core_i32.v2",
        "local_count": 1,
        "memory_slots": 1,
        "initial_memory": [
          0
        ],
        "instructions": [
          {
            "opcode": "i32_const",
            "value": 6
          },
          {
            "opcode": "i32_const",
            "value": 7
          },
          {
            "opcode": "i32_mul"
          },
          {
            "opcode": "i32_const",
            "value": 5
          },
          {
            "opcode": "i32_add"
          },
          {
            "opcode": "output"
          },
          {
            "opcode": "return"
          }
        ]
      },
      "model": {
        "schema_version": 1,
        "model_id": "alm.numeric.tassadar.alm_wasm_interpreter.v1.tassadar_corpus.mul_add_v1",
        "graph_digest": "b3ffca32a159c0662e0ff0efee45b57c5c8fc82d3d99774caaa204308b60cdfa",
        "bundle_digest": "bb618614eac8faa6715b36eede23cdda528612478149c482a8cf98b0f760d873",
        "input_field_count": 1,
        "slot_count": 60,
        "layer_count": 6,
        "seed_writes": [
          [
            0,
            0,
            0
          ],
          [
            0,
            1,
            6
          ],
          [
            0,
            2,
            0
          ],
          [
            0,
            3,
            7
          ],
          [
            0,
            4,
            5
          ],
          [
            0,
            5,
            0
          ],
          [
            0,
            6,
            0
          ],
          [
            0,
            7,
            5
          ],
          [
            0,
            8,
            3
          ],
          [
            0,
            9,
            0
          ],
          [
            0,
            10,
            10
          ],
          [
            0,
            11,
            0
          ],
          [
            0,
            12,
            11
          ],
          [
            0,
            13,
            0
          ],
          [
            0,
            14,
            11
          ],
          [
            0,
            15,
            0
          ],
          [
            0,
            16,
            11
          ],
          [
            0,
            17,
            0
          ],
          [
            1,
            -1,
            0
          ],
          [
            1,
            0,
            0
          ],
          [
            2,
            0,
            0
          ],
          [
            3,
            0,
            0
          ],
          [
            4,
            0,
            0
          ],
          [
            4,
            1,
            0
          ],
          [
            4,
            2,
            0
          ]
        ],
        "wiring": [
          {
            "out_slot": 0,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 1,
            "bias": 0,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 2,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 3,
            "bias": 2,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 10,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 11,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 14,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 16,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 17,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 18,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 19,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 20,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 21,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 22,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 23,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -11,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -1,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ],
              [
                -1,
                37
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                38
              ],
              [
                -1,
                39
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                48
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                51
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ],
              [
                -1,
                53
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": -1,
            "terms": [
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ],
              [
                -1,
                59
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 1,
            "terms": [
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                40
              ],
              [
                -1,
                41
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 21,
            "bias": 0,
            "terms": [
              [
                1,
                42
              ],
              [
                -1,
                43
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                44
              ],
              [
                -1,
                45
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 23,
            "bias": 0,
            "terms": [
              [
                1,
                46
              ],
              [
                -1,
                47
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                54
              ],
              [
                -1,
                55
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 28,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                16
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 8,
            "bias": 1,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 28,
            "bias": 1,
            "terms": [
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 29,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                7
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                20
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                20
              ],
              [
                -1,
                21
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                21
              ],
              [
                -1,
                22
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                22
              ],
              [
                -1,
                23
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                23
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                24
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                18
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                29
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                1,
                26
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 19,
            "bias": -1,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                -1,
                25
              ],
              [
                -1,
                26
              ],
              [
                -1,
                27
              ],
              [
                -1,
                35
              ],
              [
                -1,
                30
              ],
              [
                -1,
                31
              ],
              [
                -1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 21,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                1,
                30
              ],
              [
                1,
                31
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                -1,
                27
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 20,
            "bias": 1,
            "terms": [
              [
                1,
                37
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                17
              ],
              [
                1,
                8
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                1,
                18
              ],
              [
                1,
                21
              ],
              [
                1,
                22
              ],
              [
                1,
                23
              ],
              [
                1,
                24
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 10,
            "bias": -1000,
            "terms": [
              [
                1,
                26
              ],
              [
                1000,
                38
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 11,
            "bias": -1000,
            "terms": [
              [
                1,
                27
              ],
              [
                1000,
                15
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 12,
            "bias": -1000,
            "terms": [
              [
                1,
                35
              ],
              [
                1000,
                16
              ]
            ],
            "input_field": null,
            "phase": 24
          }
        ],
        "attention": [
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 1,
              "out_slot": 5,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 4,
              "out_slot": 6,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 7,
              "out_slot": 9,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 17,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 14,
              "out_slot": 18,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 8,
              "out_slot": 7,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 23,
              "out_slot": 0,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 24,
              "out_slot": 7,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 7,
            "out_slot": 9,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 35,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 36,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 38,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 39,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 17,
            "out_slot": 42,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 18,
            "out_slot": 43,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 44,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 20,
            "out_slot": 45,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 21,
            "out_slot": 46,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 22,
            "out_slot": 47,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 23,
            "out_slot": 48,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 49,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 50,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 51,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 52,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 53,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 54,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 55,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 56,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 18,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 15
          },
          {
            "value_slot": 28,
            "gate_slot": 27,
            "out_slot": 12,
            "phase": 15
          },
          {
            "value_slot": 25,
            "gate_slot": 14,
            "out_slot": 15,
            "phase": 15
          },
          {
            "value_slot": 26,
            "gate_slot": 14,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 7,
            "gate_slot": 10,
            "out_slot": 23,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 11,
            "out_slot": 24,
            "phase": 19
          },
          {
            "value_slot": 18,
            "gate_slot": 6,
            "out_slot": 25,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 26,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 27,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 36,
            "phase": 19
          },
          {
            "value_slot": 21,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 19
          },
          {
            "value_slot": 22,
            "gate_slot": 14,
            "out_slot": 38,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 28,
            "out_slot": 39,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 19
          },
          {
            "value_slot": 35,
            "gate_slot": 14,
            "out_slot": 42,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 4,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 23
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 34,
            "out_slot": 21,
            "phase": 23
          },
          {
            "value_slot": 10,
            "gate_slot": 30,
            "out_slot": 22,
            "phase": 23
          },
          {
            "value_slot": 11,
            "gate_slot": 31,
            "out_slot": 23,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 32,
            "out_slot": 24,
            "phase": 23
          },
          {
            "value_slot": 13,
            "gate_slot": 33,
            "out_slot": 25,
            "phase": 23
          },
          {
            "value_slot": 19,
            "gate_slot": 38,
            "out_slot": 26,
            "phase": 23
          },
          {
            "value_slot": 40,
            "gate_slot": 15,
            "out_slot": 27,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 15,
            "out_slot": 28,
            "phase": 23
          },
          {
            "value_slot": 41,
            "gate_slot": 16,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 16,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 42,
            "out_slot": 37,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 10,
            "value_slot": 7
          },
          {
            "channel": 2,
            "key_slot": 11,
            "value_slot": 28
          },
          {
            "channel": 3,
            "key_slot": 12,
            "value_slot": 36
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 0
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 19
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 6
          }
        ],
        "output_slots": [
          42,
          37,
          0,
          19,
          6
        ]
      },
      "steps": [
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ]
      ],
      "expectedTraceDigest": "1a47828b750fc9c964f631f729766c2ebc0897b2e65ae1c84c1170a7b08f2af6",
      "expectedModelDigest": "51733079031bc94ed0989ff0a72e48a38b1744767357661d10c9d6c440f793ec",
      "expectedFinalRow": [
        0,
        0,
        7,
        0,
        1
      ],
      "expectedOutputs": [
        47
      ],
      "halted": true,
      "compileReceiptRefs": [
        "receipt.psionic.tassadar_program.3a90befea6efb5f7",
        "receipt.psionic.tassadar_graph.b3ffca32a159c066",
        "receipt.psionic.tassadar_bundle.bb618614eac8faa6",
        "receipt.psionic.tassadar_numeric_model.51733079031bc94e",
        "receipt.psionic.tassadar_trace.1a47828b750fc9c9"
      ]
    },
    {
      "fixtureId": "tassadar_corpus.memory_roundtrip_v1.numeric_fixture.v1",
      "programId": "tassadar_corpus.memory_roundtrip_v1",
      "programDigest": "63cc87aa90bbcbfd0108105f4b5974160f74cd24ef4600252cbdec59a6727199",
      "workloadKind": "memory.load_store_roundtrip",
      "profileId": "tassadar.wasm.core_i32.v2",
      "program": {
        "program_id": "tassadar_corpus.memory_roundtrip_v1",
        "profile_id": "tassadar.wasm.core_i32.v2",
        "local_count": 1,
        "memory_slots": 2,
        "initial_memory": [
          37,
          0
        ],
        "instructions": [
          {
            "opcode": "i32_load",
            "slot": 0
          },
          {
            "opcode": "i32_const",
            "value": 5
          },
          {
            "opcode": "i32_add"
          },
          {
            "opcode": "i32_store",
            "slot": 1
          },
          {
            "opcode": "i32_load",
            "slot": 1
          },
          {
            "opcode": "output"
          },
          {
            "opcode": "return"
          }
        ]
      },
      "model": {
        "schema_version": 1,
        "model_id": "alm.numeric.tassadar.alm_wasm_interpreter.v1.tassadar_corpus.memory_roundtrip_v1",
        "graph_digest": "df5267de2d1573cc1ce5d469c9705c4ef8d3d8eecdca767202d13963919e6878",
        "bundle_digest": "fac8ba5c50a5b13c22270695e77b00db8e6102c5d8bfab90252f0bd0948713ad",
        "input_field_count": 1,
        "slot_count": 60,
        "layer_count": 6,
        "seed_writes": [
          [
            0,
            0,
            7
          ],
          [
            0,
            1,
            0
          ],
          [
            0,
            2,
            0
          ],
          [
            0,
            3,
            5
          ],
          [
            0,
            4,
            3
          ],
          [
            0,
            5,
            0
          ],
          [
            0,
            6,
            8
          ],
          [
            0,
            7,
            1
          ],
          [
            0,
            8,
            7
          ],
          [
            0,
            9,
            1
          ],
          [
            0,
            10,
            10
          ],
          [
            0,
            11,
            0
          ],
          [
            0,
            12,
            11
          ],
          [
            0,
            13,
            0
          ],
          [
            0,
            14,
            11
          ],
          [
            0,
            15,
            0
          ],
          [
            0,
            16,
            11
          ],
          [
            0,
            17,
            0
          ],
          [
            1,
            -1,
            0
          ],
          [
            1,
            0,
            0
          ],
          [
            2,
            0,
            0
          ],
          [
            3,
            0,
            37
          ],
          [
            3,
            1,
            0
          ],
          [
            4,
            0,
            0
          ],
          [
            4,
            1,
            0
          ],
          [
            4,
            2,
            0
          ]
        ],
        "wiring": [
          {
            "out_slot": 0,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 1,
            "bias": 0,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 2,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 3,
            "bias": 2,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 10,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 11,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 14,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 16,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 17,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 18,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 19,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 20,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 21,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 22,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 23,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -11,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -1,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ],
              [
                -1,
                37
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                38
              ],
              [
                -1,
                39
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                48
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                51
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ],
              [
                -1,
                53
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": -1,
            "terms": [
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ],
              [
                -1,
                59
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 1,
            "terms": [
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                40
              ],
              [
                -1,
                41
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 21,
            "bias": 0,
            "terms": [
              [
                1,
                42
              ],
              [
                -1,
                43
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                44
              ],
              [
                -1,
                45
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 23,
            "bias": 0,
            "terms": [
              [
                1,
                46
              ],
              [
                -1,
                47
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                54
              ],
              [
                -1,
                55
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 28,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                16
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 8,
            "bias": 1,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 28,
            "bias": 1,
            "terms": [
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 29,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                7
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                20
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                20
              ],
              [
                -1,
                21
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                21
              ],
              [
                -1,
                22
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                22
              ],
              [
                -1,
                23
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                23
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                24
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                18
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                29
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                1,
                26
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 19,
            "bias": -1,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                -1,
                25
              ],
              [
                -1,
                26
              ],
              [
                -1,
                27
              ],
              [
                -1,
                35
              ],
              [
                -1,
                30
              ],
              [
                -1,
                31
              ],
              [
                -1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 21,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                1,
                30
              ],
              [
                1,
                31
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                -1,
                27
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 20,
            "bias": 1,
            "terms": [
              [
                1,
                37
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                17
              ],
              [
                1,
                8
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                1,
                18
              ],
              [
                1,
                21
              ],
              [
                1,
                22
              ],
              [
                1,
                23
              ],
              [
                1,
                24
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 10,
            "bias": -1000,
            "terms": [
              [
                1,
                26
              ],
              [
                1000,
                38
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 11,
            "bias": -1000,
            "terms": [
              [
                1,
                27
              ],
              [
                1000,
                15
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 12,
            "bias": -1000,
            "terms": [
              [
                1,
                35
              ],
              [
                1000,
                16
              ]
            ],
            "input_field": null,
            "phase": 24
          }
        ],
        "attention": [
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 1,
              "out_slot": 5,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 4,
              "out_slot": 6,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 7,
              "out_slot": 9,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 17,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 14,
              "out_slot": 18,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 8,
              "out_slot": 7,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 23,
              "out_slot": 0,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 24,
              "out_slot": 7,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 7,
            "out_slot": 9,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 35,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 36,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 38,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 39,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 17,
            "out_slot": 42,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 18,
            "out_slot": 43,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 44,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 20,
            "out_slot": 45,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 21,
            "out_slot": 46,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 22,
            "out_slot": 47,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 23,
            "out_slot": 48,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 49,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 50,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 51,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 52,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 53,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 54,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 55,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 56,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 18,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 15
          },
          {
            "value_slot": 28,
            "gate_slot": 27,
            "out_slot": 12,
            "phase": 15
          },
          {
            "value_slot": 25,
            "gate_slot": 14,
            "out_slot": 15,
            "phase": 15
          },
          {
            "value_slot": 26,
            "gate_slot": 14,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 7,
            "gate_slot": 10,
            "out_slot": 23,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 11,
            "out_slot": 24,
            "phase": 19
          },
          {
            "value_slot": 18,
            "gate_slot": 6,
            "out_slot": 25,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 26,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 27,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 36,
            "phase": 19
          },
          {
            "value_slot": 21,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 19
          },
          {
            "value_slot": 22,
            "gate_slot": 14,
            "out_slot": 38,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 28,
            "out_slot": 39,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 19
          },
          {
            "value_slot": 35,
            "gate_slot": 14,
            "out_slot": 42,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 4,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 23
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 34,
            "out_slot": 21,
            "phase": 23
          },
          {
            "value_slot": 10,
            "gate_slot": 30,
            "out_slot": 22,
            "phase": 23
          },
          {
            "value_slot": 11,
            "gate_slot": 31,
            "out_slot": 23,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 32,
            "out_slot": 24,
            "phase": 23
          },
          {
            "value_slot": 13,
            "gate_slot": 33,
            "out_slot": 25,
            "phase": 23
          },
          {
            "value_slot": 19,
            "gate_slot": 38,
            "out_slot": 26,
            "phase": 23
          },
          {
            "value_slot": 40,
            "gate_slot": 15,
            "out_slot": 27,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 15,
            "out_slot": 28,
            "phase": 23
          },
          {
            "value_slot": 41,
            "gate_slot": 16,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 16,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 42,
            "out_slot": 37,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 10,
            "value_slot": 7
          },
          {
            "channel": 2,
            "key_slot": 11,
            "value_slot": 28
          },
          {
            "channel": 3,
            "key_slot": 12,
            "value_slot": 36
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 0
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 19
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 6
          }
        ],
        "output_slots": [
          42,
          37,
          0,
          19,
          6
        ]
      },
      "steps": [
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ]
      ],
      "expectedTraceDigest": "467637249759afbcdc693a6e86f6252e220a39eee8f51887e6d674c75f85cc18",
      "expectedModelDigest": "4c86ba88c1211ccf655040df6a5d01d82367f4aa57241df964890bb18eb09b22",
      "expectedFinalRow": [
        0,
        0,
        7,
        0,
        1
      ],
      "expectedOutputs": [
        42
      ],
      "halted": true,
      "compileReceiptRefs": [
        "receipt.psionic.tassadar_program.63cc87aa90bbcbfd",
        "receipt.psionic.tassadar_graph.df5267de2d1573cc",
        "receipt.psionic.tassadar_bundle.fac8ba5c50a5b13c",
        "receipt.psionic.tassadar_numeric_model.4c86ba88c1211ccf",
        "receipt.psionic.tassadar_trace.467637249759afbc"
      ]
    },
    {
      "fixtureId": "tassadar_corpus.factorial_loop_v1.numeric_fixture.v1",
      "programId": "tassadar_corpus.factorial_loop_v1",
      "programDigest": "3ce8f0b7ed2fbbd697e5064ad6bbac9225a44825d3b0e27f3179974d0ab110af",
      "workloadKind": "state_machine.factorial_countdown",
      "profileId": "tassadar.wasm.article_i32_compute.v1",
      "program": {
        "program_id": "tassadar_corpus.factorial_loop_v1",
        "profile_id": "tassadar.wasm.article_i32_compute.v1",
        "local_count": 2,
        "memory_slots": 1,
        "initial_memory": [
          0
        ],
        "instructions": [
          {
            "opcode": "i32_const",
            "value": 4
          },
          {
            "opcode": "local_set",
            "local": 0
          },
          {
            "opcode": "i32_const",
            "value": 1
          },
          {
            "opcode": "local_set",
            "local": 1
          },
          {
            "opcode": "local_get",
            "local": 1
          },
          {
            "opcode": "local_get",
            "local": 0
          },
          {
            "opcode": "i32_mul"
          },
          {
            "opcode": "local_set",
            "local": 1
          },
          {
            "opcode": "local_get",
            "local": 0
          },
          {
            "opcode": "i32_const",
            "value": 1
          },
          {
            "opcode": "i32_sub"
          },
          {
            "opcode": "local_set",
            "local": 0
          },
          {
            "opcode": "i32_const",
            "value": 1
          },
          {
            "opcode": "local_get",
            "local": 0
          },
          {
            "opcode": "i32_lt"
          },
          {
            "opcode": "br_if",
            "target_pc": 4
          },
          {
            "opcode": "local_get",
            "local": 1
          },
          {
            "opcode": "output"
          },
          {
            "opcode": "return"
          }
        ]
      },
      "model": {
        "schema_version": 1,
        "model_id": "alm.numeric.tassadar.alm_wasm_interpreter.v1.tassadar_corpus.factorial_loop_v1",
        "graph_digest": "5a98e6c54901d55c2a74aa5eb8e14a55abcafcc8263fcf09c320ed571e18b4bb",
        "bundle_digest": "d7bfb2c775362a9a16664f876ae07855a5dea3702b837fdd20344f58487ced03",
        "input_field_count": 1,
        "slot_count": 60,
        "layer_count": 6,
        "seed_writes": [
          [
            0,
            0,
            0
          ],
          [
            0,
            1,
            4
          ],
          [
            0,
            2,
            2
          ],
          [
            0,
            3,
            0
          ],
          [
            0,
            4,
            0
          ],
          [
            0,
            5,
            1
          ],
          [
            0,
            6,
            2
          ],
          [
            0,
            7,
            1
          ],
          [
            0,
            8,
            1
          ],
          [
            0,
            9,
            1
          ],
          [
            0,
            10,
            1
          ],
          [
            0,
            11,
            0
          ],
          [
            0,
            12,
            5
          ],
          [
            0,
            13,
            0
          ],
          [
            0,
            14,
            2
          ],
          [
            0,
            15,
            1
          ],
          [
            0,
            16,
            1
          ],
          [
            0,
            17,
            0
          ],
          [
            0,
            18,
            0
          ],
          [
            0,
            19,
            1
          ],
          [
            0,
            20,
            4
          ],
          [
            0,
            21,
            0
          ],
          [
            0,
            22,
            2
          ],
          [
            0,
            23,
            0
          ],
          [
            0,
            24,
            0
          ],
          [
            0,
            25,
            1
          ],
          [
            0,
            26,
            1
          ],
          [
            0,
            27,
            0
          ],
          [
            0,
            28,
            6
          ],
          [
            0,
            29,
            0
          ],
          [
            0,
            30,
            9
          ],
          [
            0,
            31,
            4
          ],
          [
            0,
            32,
            1
          ],
          [
            0,
            33,
            1
          ],
          [
            0,
            34,
            10
          ],
          [
            0,
            35,
            0
          ],
          [
            0,
            36,
            11
          ],
          [
            0,
            37,
            0
          ],
          [
            0,
            38,
            11
          ],
          [
            0,
            39,
            0
          ],
          [
            0,
            40,
            11
          ],
          [
            0,
            41,
            0
          ],
          [
            1,
            -1,
            0
          ],
          [
            1,
            0,
            0
          ],
          [
            2,
            0,
            0
          ],
          [
            2,
            1,
            0
          ],
          [
            3,
            0,
            0
          ],
          [
            4,
            0,
            0
          ],
          [
            4,
            1,
            0
          ],
          [
            4,
            2,
            0
          ]
        ],
        "wiring": [
          {
            "out_slot": 0,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 1,
            "bias": 0,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 2,
            "bias": 1,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 3,
            "bias": 2,
            "terms": [],
            "input_field": null,
            "phase": 0
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 10,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 11,
            "bias": -1,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": -2,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 14,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -3,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 16,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 17,
            "bias": -4,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 18,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 19,
            "bias": -5,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 20,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 21,
            "bias": -6,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 22,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 23,
            "bias": -7,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -8,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -9,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -10,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -11,
            "terms": [
              [
                1,
                9
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -1,
            "terms": [
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ],
              [
                -1,
                37
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                38
              ],
              [
                -1,
                39
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                48
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                51
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ],
              [
                -1,
                53
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": -1,
            "terms": [
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ],
              [
                -1,
                59
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 1,
            "terms": [
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                9
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                40
              ],
              [
                -1,
                41
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 21,
            "bias": 0,
            "terms": [
              [
                1,
                42
              ],
              [
                -1,
                43
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                44
              ],
              [
                -1,
                45
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 23,
            "bias": 0,
            "terms": [
              [
                1,
                46
              ],
              [
                -1,
                47
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                54
              ],
              [
                -1,
                55
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 28,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                16
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 8,
            "bias": 1,
            "terms": [
              [
                2,
                5
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 28,
            "bias": 1,
            "terms": [
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 29,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                7
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                20
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                20
              ],
              [
                -1,
                21
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                21
              ],
              [
                -1,
                22
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                22
              ],
              [
                -1,
                23
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                23
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                24
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                18
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                29
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                1,
                26
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 19,
            "bias": -1,
            "terms": [
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 20,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                -1,
                25
              ],
              [
                -1,
                26
              ],
              [
                -1,
                27
              ],
              [
                -1,
                35
              ],
              [
                -1,
                30
              ],
              [
                -1,
                31
              ],
              [
                -1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 21,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 22,
            "bias": 0,
            "terms": [
              [
                1,
                28
              ],
              [
                1,
                29
              ],
              [
                1,
                34
              ],
              [
                1,
                30
              ],
              [
                1,
                31
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 10,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 11,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                -1,
                27
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 19,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 20,
            "bias": 1,
            "terms": [
              [
                1,
                37
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                4
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                17
              ],
              [
                1,
                8
              ],
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 7,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                1,
                18
              ],
              [
                1,
                21
              ],
              [
                1,
                22
              ],
              [
                1,
                23
              ],
              [
                1,
                24
              ],
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 10,
            "bias": -1000,
            "terms": [
              [
                1,
                26
              ],
              [
                1000,
                38
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 11,
            "bias": -1000,
            "terms": [
              [
                1,
                27
              ],
              [
                1000,
                15
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 12,
            "bias": -1000,
            "terms": [
              [
                1,
                35
              ],
              [
                1000,
                16
              ]
            ],
            "input_field": null,
            "phase": 24
          }
        ],
        "attention": [
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 1,
              "out_slot": 5,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 4,
              "out_slot": 6,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 7,
              "out_slot": 9,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 17,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 14,
              "out_slot": 18,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 8,
              "out_slot": 7,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 23,
              "out_slot": 0,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 24,
              "out_slot": 7,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 7,
            "out_slot": 9,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 35,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 36,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 38,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 39,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 17,
            "out_slot": 42,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 18,
            "out_slot": 43,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 44,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 20,
            "out_slot": 45,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 21,
            "out_slot": 46,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 22,
            "out_slot": 47,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 23,
            "out_slot": 48,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 49,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 50,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 51,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 52,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 53,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 54,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 55,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 56,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 18,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 15
          },
          {
            "value_slot": 28,
            "gate_slot": 27,
            "out_slot": 12,
            "phase": 15
          },
          {
            "value_slot": 25,
            "gate_slot": 14,
            "out_slot": 15,
            "phase": 15
          },
          {
            "value_slot": 26,
            "gate_slot": 14,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 7,
            "gate_slot": 10,
            "out_slot": 23,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 11,
            "out_slot": 24,
            "phase": 19
          },
          {
            "value_slot": 18,
            "gate_slot": 6,
            "out_slot": 25,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 26,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 19,
            "out_slot": 27,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 36,
            "phase": 19
          },
          {
            "value_slot": 21,
            "gate_slot": 12,
            "out_slot": 37,
            "phase": 19
          },
          {
            "value_slot": 22,
            "gate_slot": 14,
            "out_slot": 38,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 28,
            "out_slot": 39,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 15,
            "out_slot": 40,
            "phase": 19
          },
          {
            "value_slot": 7,
            "gate_slot": 16,
            "out_slot": 41,
            "phase": 19
          },
          {
            "value_slot": 35,
            "gate_slot": 14,
            "out_slot": 42,
            "phase": 19
          },
          {
            "value_slot": 20,
            "gate_slot": 14,
            "out_slot": 4,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 8,
            "out_slot": 9,
            "phase": 23
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 34,
            "out_slot": 21,
            "phase": 23
          },
          {
            "value_slot": 10,
            "gate_slot": 30,
            "out_slot": 22,
            "phase": 23
          },
          {
            "value_slot": 11,
            "gate_slot": 31,
            "out_slot": 23,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 32,
            "out_slot": 24,
            "phase": 23
          },
          {
            "value_slot": 13,
            "gate_slot": 33,
            "out_slot": 25,
            "phase": 23
          },
          {
            "value_slot": 19,
            "gate_slot": 38,
            "out_slot": 26,
            "phase": 23
          },
          {
            "value_slot": 40,
            "gate_slot": 15,
            "out_slot": 27,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 15,
            "out_slot": 28,
            "phase": 23
          },
          {
            "value_slot": 41,
            "gate_slot": 16,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 16,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 42,
            "out_slot": 37,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 10,
            "value_slot": 7
          },
          {
            "channel": 2,
            "key_slot": 11,
            "value_slot": 28
          },
          {
            "channel": 3,
            "key_slot": 12,
            "value_slot": 36
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 0
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 19
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 6
          }
        ],
        "output_slots": [
          42,
          37,
          0,
          19,
          6
        ]
      },
      "steps": [
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ],
        [
          0
        ]
      ],
      "expectedTraceDigest": "b1b48c92b0e5aaca203832361690e552d475cb18ebf805c3cb458f464fcee98a",
      "expectedModelDigest": "db2e99920a3a6e6681b06c5a5bf1d52f5ed9d167e8fff9c3498327b8a9157c8b",
      "expectedFinalRow": [
        0,
        0,
        19,
        0,
        1
      ],
      "expectedOutputs": [
        24
      ],
      "halted": true,
      "compileReceiptRefs": [
        "receipt.psionic.tassadar_program.3ce8f0b7ed2fbbd6",
        "receipt.psionic.tassadar_graph.5a98e6c54901d55c",
        "receipt.psionic.tassadar_bundle.d7bfb2c775362a9a",
        "receipt.psionic.tassadar_numeric_model.db2e99920a3a6e66",
        "receipt.psionic.tassadar_trace.b1b48c92b0e5aaca"
      ]
    }
  ],
  "corpusDigest": "1b7babcd0c3ce63e43212f3e4f07480969a7a9612a237b117f8de7fb8a828d6a"
}

const stableWorkloadIndex = (seed: string): number => {
  let hash = 2166136261
  for (let index = 0; index < seed.length; index += 1) {
    hash ^= seed.charCodeAt(index)
    hash = Math.imul(hash, 16777619) >>> 0
  }
  return hash % tassadarCompiledProgramCorpus.fixtures.length
}

const explicitWorkloadIndex = (assignmentRef: string): number | null => {
  const marker = ".w"
  const markerIndex = assignmentRef.lastIndexOf(marker)
  if (markerIndex < 0) {
    return null
  }
  const suffix = assignmentRef.slice(markerIndex + marker.length)
  if (!/^[0-9]+$/.test(suffix)) {
    return null
  }
  return Number(suffix) % tassadarCompiledProgramCorpus.fixtures.length
}

export const tassadarCompiledProgramCorpusSize =
  tassadarCompiledProgramCorpus.fixtures.length

export const selectTassadarCompiledProgramFixture = (input: Readonly<{
  assignmentRef: string
}>): TassadarCompiledProgramFixture => {
  const index =
    explicitWorkloadIndex(input.assignmentRef) ??
    (input.assignmentRef.startsWith("assignment.artanis_admin.")
      ? 0
      : stableWorkloadIndex(input.assignmentRef))
  return tassadarCompiledProgramCorpus.fixtures[index] ?? tassadarCompiledProgramCorpus.fixtures[0]!
}
