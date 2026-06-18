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
  "programCount": 5,
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
        "graph_digest": "88219d6cd782a035c2979bdc2bcc00b79dbe6152dd8e918534d7c373b18caa1c",
        "bundle_digest": "226022f654a907e7ea994ef42fffa5b5d0bde1c68a4b52bfbdca875d8766d3a3",
        "input_field_count": 1,
        "slot_count": 83,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 4
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 9,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 10,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 11,
            "bias": -3,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 13,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 14,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 15,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                7
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 35,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 36,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 37,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 38,
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
            "phase": 10
          },
          {
            "out_slot": 39,
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
            "phase": 10
          },
          {
            "out_slot": 40,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 41,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 42,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 43,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 44,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 45,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 46,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 47,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 48,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 49,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 50,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 51,
            "bias": -20,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 52,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 53,
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
            "out_slot": 54,
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
            "out_slot": 55,
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
            "out_slot": 56,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                59
              ],
              [
                -1,
                60
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
                61
              ],
              [
                -1,
                62
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                63
              ],
              [
                -1,
                64
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
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 24,
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
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                -1,
                52
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                79
              ],
              [
                -1,
                80
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                81
              ],
              [
                -1,
                82
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 29,
            "bias": 1,
            "terms": [
              [
                -1,
                28
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 32,
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
            "phase": 14
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
            "phase": 14
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                57
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                65
              ],
              [
                -1,
                66
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                67
              ],
              [
                -1,
                68
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 37,
            "bias": 0,
            "terms": [
              [
                1,
                69
              ],
              [
                -1,
                70
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 40,
            "bias": 0,
            "terms": [
              [
                1,
                71
              ],
              [
                -1,
                72
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
              [
                1,
                73
              ],
              [
                -1,
                74
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 42,
            "bias": 0,
            "terms": [
              [
                1,
                75
              ],
              [
                -1,
                76
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
              [
                1,
                77
              ],
              [
                -1,
                78
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 44,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 45,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                14
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 47,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 49,
            "bias": -1,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 50,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                1,
                27
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 51,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 9,
            "bias": 1,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                30
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                30
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                31
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                31
              ],
              [
                -1,
                32
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                33
              ],
              [
                -1,
                34
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 47,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                14
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 49,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                -1,
                36
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 51,
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
            "phase": 16
          },
          {
            "out_slot": 53,
            "bias": 0,
            "terms": [
              [
                1,
                37
              ],
              [
                -1,
                40
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 54,
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
            "phase": 16
          },
          {
            "out_slot": 55,
            "bias": 0,
            "terms": [
              [
                1,
                41
              ],
              [
                -1,
                42
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 56,
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
            "phase": 16
          },
          {
            "out_slot": 57,
            "bias": 0,
            "terms": [
              [
                1,
                43
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 58,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 9,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
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
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                47
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 31,
            "bias": -1,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 36,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 37,
            "bias": 1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 39,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 40,
            "bias": 1,
            "terms": [
              [
                -1,
                50
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                -1,
                15
              ],
              [
                -1,
                44
              ],
              [
                -1,
                45
              ],
              [
                -1,
                48
              ],
              [
                -1,
                49
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
                46
              ],
              [
                -1,
                53
              ],
              [
                -1,
                54
              ],
              [
                -1,
                55
              ],
              [
                -1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 42,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                4
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                1,
                25
              ],
              [
                1,
                26
              ],
              [
                1,
                27
              ],
              [
                1,
                46
              ],
              [
                1,
                51
              ],
              [
                1,
                53
              ],
              [
                1,
                54
              ],
              [
                1,
                55
              ],
              [
                1,
                56
              ],
              [
                1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                15
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
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
            "phase": 20
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                1,
                45
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                35
              ],
              [
                -1,
                0
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                0
              ],
              [
                -1,
                38
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 17,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                44
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
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                59
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 31,
            "bias": 1,
            "terms": [
              [
                1,
                60
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                0
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
                28
              ],
              [
                1,
                13
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                62
              ],
              [
                1,
                10
              ],
              [
                1,
                11
              ],
              [
                1,
                14
              ],
              [
                1,
                63
              ],
              [
                1,
                18
              ],
              [
                1,
                19
              ],
              [
                1,
                64
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ],
              [
                1,
                34
              ],
              [
                1,
                35
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 9,
            "bias": -1000,
            "terms": [
              [
                1,
                37
              ],
              [
                1000,
                61
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
                38
              ],
              [
                1000,
                21
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 15,
            "bias": -1000,
            "terms": [
              [
                1,
                40
              ],
              [
                1000,
                22
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
              "query_slot": 1,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 5,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 6,
              "out_slot": 7,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 5,
              "out_slot": 8,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 6,
              "out_slot": 10,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 28,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 9,
              "out_slot": 6,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 16,
              "out_slot": 7,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 17,
              "out_slot": 12,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 17,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 18,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 19,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 20,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 21,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 22,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 23,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 17,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 18,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 19,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 20,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 21,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 22,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 23,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 60,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 61,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 62,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 63,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 35,
            "out_slot": 64,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 65,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 66,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 40,
            "out_slot": 67,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 41,
            "out_slot": 68,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 42,
            "out_slot": 69,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 43,
            "out_slot": 70,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 44,
            "out_slot": 71,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 45,
            "out_slot": 72,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 73,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 74,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 75,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 76,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 50,
            "out_slot": 77,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 51,
            "out_slot": 78,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 53,
            "out_slot": 79,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 54,
            "out_slot": 80,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 55,
            "out_slot": 81,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 56,
            "out_slot": 82,
            "phase": 11
          },
          {
            "value_slot": 10,
            "gate_slot": 9,
            "out_slot": 11,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 17,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 18,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 19,
            "phase": 15
          },
          {
            "value_slot": 50,
            "gate_slot": 45,
            "out_slot": 20,
            "phase": 15
          },
          {
            "value_slot": 51,
            "gate_slot": 29,
            "out_slot": 21,
            "phase": 15
          },
          {
            "value_slot": 44,
            "gate_slot": 29,
            "out_slot": 22,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 7,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 15,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 13,
            "out_slot": 16,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 14,
            "out_slot": 17,
            "phase": 19
          },
          {
            "value_slot": 10,
            "gate_slot": 8,
            "out_slot": 18,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 19,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 44,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 45,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 49,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 50,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 39,
            "out_slot": 58,
            "phase": 19
          },
          {
            "value_slot": 41,
            "gate_slot": 29,
            "out_slot": 59,
            "phase": 19
          },
          {
            "value_slot": 42,
            "gate_slot": 20,
            "out_slot": 60,
            "phase": 19
          },
          {
            "value_slot": 43,
            "gate_slot": 29,
            "out_slot": 61,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 23,
            "out_slot": 62,
            "phase": 19
          },
          {
            "value_slot": 52,
            "gate_slot": 26,
            "out_slot": 63,
            "phase": 19
          },
          {
            "value_slot": 40,
            "gate_slot": 51,
            "out_slot": 64,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 21,
            "out_slot": 65,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 22,
            "out_slot": 66,
            "phase": 19
          },
          {
            "value_slot": 48,
            "gate_slot": 29,
            "out_slot": 67,
            "phase": 19
          },
          {
            "value_slot": 31,
            "gate_slot": 29,
            "out_slot": 0,
            "phase": 23
          },
          {
            "value_slot": 28,
            "gate_slot": 13,
            "out_slot": 5,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 24,
            "out_slot": 10,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 47,
            "out_slot": 11,
            "phase": 23
          },
          {
            "value_slot": 15,
            "gate_slot": 25,
            "out_slot": 14,
            "phase": 23
          },
          {
            "value_slot": 16,
            "gate_slot": 27,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 46,
            "out_slot": 19,
            "phase": 23
          },
          {
            "value_slot": 20,
            "gate_slot": 53,
            "out_slot": 32,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 54,
            "out_slot": 33,
            "phase": 23
          },
          {
            "value_slot": 9,
            "gate_slot": 55,
            "out_slot": 34,
            "phase": 23
          },
          {
            "value_slot": 23,
            "gate_slot": 56,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 26,
            "gate_slot": 57,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 30,
            "gate_slot": 61,
            "out_slot": 37,
            "phase": 23
          },
          {
            "value_slot": 65,
            "gate_slot": 21,
            "out_slot": 38,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 21,
            "out_slot": 39,
            "phase": 23
          },
          {
            "value_slot": 66,
            "gate_slot": 22,
            "out_slot": 40,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 22,
            "out_slot": 41,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 67,
            "out_slot": 42,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 9,
            "value_slot": 8
          },
          {
            "channel": 2,
            "key_slot": 12,
            "value_slot": 39
          },
          {
            "channel": 3,
            "key_slot": 15,
            "value_slot": 41
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 6
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 30
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 7
          }
        ],
        "output_slots": [
          67,
          42,
          6,
          30,
          7
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
      "expectedTraceDigest": "b012fdddd616fba9e3707f7dd688bfc2397f8a953decbdd7e9e4c2e8023b04a1",
      "expectedModelDigest": "35f0bae7f97df0c89d48f66b7b4af43aef5b9bf4d8ffbce6687cc1d97d8a375f",
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
        "receipt.psionic.tassadar_graph.88219d6cd782a035",
        "receipt.psionic.tassadar_bundle.226022f654a907e7",
        "receipt.psionic.tassadar_numeric_model.35f0bae7f97df0c8",
        "receipt.psionic.tassadar_trace.b012fdddd616fba9"
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
        "graph_digest": "6ba49df561640d53f1f35cda5a582e7137dba34a739b45a86d88b4999af2547b",
        "bundle_digest": "60eea91f657d636f362ce6a3c278492260a26ac2ebde685cdcb4cc8bb78b1668",
        "input_field_count": 1,
        "slot_count": 83,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 4
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 9,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 10,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 11,
            "bias": -3,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 13,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 14,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 15,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                7
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 35,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 36,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 37,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 38,
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
            "phase": 10
          },
          {
            "out_slot": 39,
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
            "phase": 10
          },
          {
            "out_slot": 40,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 41,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 42,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 43,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 44,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 45,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 46,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 47,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 48,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 49,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 50,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 51,
            "bias": -20,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 52,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 53,
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
            "out_slot": 54,
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
            "out_slot": 55,
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
            "out_slot": 56,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                59
              ],
              [
                -1,
                60
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
                61
              ],
              [
                -1,
                62
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                63
              ],
              [
                -1,
                64
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
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 24,
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
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                -1,
                52
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                79
              ],
              [
                -1,
                80
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                81
              ],
              [
                -1,
                82
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 29,
            "bias": 1,
            "terms": [
              [
                -1,
                28
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 32,
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
            "phase": 14
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
            "phase": 14
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                57
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                65
              ],
              [
                -1,
                66
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                67
              ],
              [
                -1,
                68
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 37,
            "bias": 0,
            "terms": [
              [
                1,
                69
              ],
              [
                -1,
                70
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 40,
            "bias": 0,
            "terms": [
              [
                1,
                71
              ],
              [
                -1,
                72
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
              [
                1,
                73
              ],
              [
                -1,
                74
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 42,
            "bias": 0,
            "terms": [
              [
                1,
                75
              ],
              [
                -1,
                76
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
              [
                1,
                77
              ],
              [
                -1,
                78
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 44,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 45,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                14
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 47,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 49,
            "bias": -1,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 50,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                1,
                27
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 51,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 9,
            "bias": 1,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                30
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                30
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                31
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                31
              ],
              [
                -1,
                32
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                33
              ],
              [
                -1,
                34
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 47,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                14
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 49,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                -1,
                36
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 51,
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
            "phase": 16
          },
          {
            "out_slot": 53,
            "bias": 0,
            "terms": [
              [
                1,
                37
              ],
              [
                -1,
                40
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 54,
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
            "phase": 16
          },
          {
            "out_slot": 55,
            "bias": 0,
            "terms": [
              [
                1,
                41
              ],
              [
                -1,
                42
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 56,
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
            "phase": 16
          },
          {
            "out_slot": 57,
            "bias": 0,
            "terms": [
              [
                1,
                43
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 58,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 9,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
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
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                47
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 31,
            "bias": -1,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 36,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 37,
            "bias": 1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 39,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 40,
            "bias": 1,
            "terms": [
              [
                -1,
                50
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                -1,
                15
              ],
              [
                -1,
                44
              ],
              [
                -1,
                45
              ],
              [
                -1,
                48
              ],
              [
                -1,
                49
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
                46
              ],
              [
                -1,
                53
              ],
              [
                -1,
                54
              ],
              [
                -1,
                55
              ],
              [
                -1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 42,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                4
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                1,
                25
              ],
              [
                1,
                26
              ],
              [
                1,
                27
              ],
              [
                1,
                46
              ],
              [
                1,
                51
              ],
              [
                1,
                53
              ],
              [
                1,
                54
              ],
              [
                1,
                55
              ],
              [
                1,
                56
              ],
              [
                1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                15
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
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
            "phase": 20
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                1,
                45
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                35
              ],
              [
                -1,
                0
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                0
              ],
              [
                -1,
                38
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 17,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                44
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
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                59
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 31,
            "bias": 1,
            "terms": [
              [
                1,
                60
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                0
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
                28
              ],
              [
                1,
                13
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                62
              ],
              [
                1,
                10
              ],
              [
                1,
                11
              ],
              [
                1,
                14
              ],
              [
                1,
                63
              ],
              [
                1,
                18
              ],
              [
                1,
                19
              ],
              [
                1,
                64
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ],
              [
                1,
                34
              ],
              [
                1,
                35
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 9,
            "bias": -1000,
            "terms": [
              [
                1,
                37
              ],
              [
                1000,
                61
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
                38
              ],
              [
                1000,
                21
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 15,
            "bias": -1000,
            "terms": [
              [
                1,
                40
              ],
              [
                1000,
                22
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
              "query_slot": 1,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 5,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 6,
              "out_slot": 7,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 5,
              "out_slot": 8,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 6,
              "out_slot": 10,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 28,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 9,
              "out_slot": 6,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 16,
              "out_slot": 7,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 17,
              "out_slot": 12,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 17,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 18,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 19,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 20,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 21,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 22,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 23,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 17,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 18,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 19,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 20,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 21,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 22,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 23,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 60,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 61,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 62,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 63,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 35,
            "out_slot": 64,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 65,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 66,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 40,
            "out_slot": 67,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 41,
            "out_slot": 68,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 42,
            "out_slot": 69,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 43,
            "out_slot": 70,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 44,
            "out_slot": 71,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 45,
            "out_slot": 72,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 73,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 74,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 75,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 76,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 50,
            "out_slot": 77,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 51,
            "out_slot": 78,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 53,
            "out_slot": 79,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 54,
            "out_slot": 80,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 55,
            "out_slot": 81,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 56,
            "out_slot": 82,
            "phase": 11
          },
          {
            "value_slot": 10,
            "gate_slot": 9,
            "out_slot": 11,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 17,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 18,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 19,
            "phase": 15
          },
          {
            "value_slot": 50,
            "gate_slot": 45,
            "out_slot": 20,
            "phase": 15
          },
          {
            "value_slot": 51,
            "gate_slot": 29,
            "out_slot": 21,
            "phase": 15
          },
          {
            "value_slot": 44,
            "gate_slot": 29,
            "out_slot": 22,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 7,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 15,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 13,
            "out_slot": 16,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 14,
            "out_slot": 17,
            "phase": 19
          },
          {
            "value_slot": 10,
            "gate_slot": 8,
            "out_slot": 18,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 19,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 44,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 45,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 49,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 50,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 39,
            "out_slot": 58,
            "phase": 19
          },
          {
            "value_slot": 41,
            "gate_slot": 29,
            "out_slot": 59,
            "phase": 19
          },
          {
            "value_slot": 42,
            "gate_slot": 20,
            "out_slot": 60,
            "phase": 19
          },
          {
            "value_slot": 43,
            "gate_slot": 29,
            "out_slot": 61,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 23,
            "out_slot": 62,
            "phase": 19
          },
          {
            "value_slot": 52,
            "gate_slot": 26,
            "out_slot": 63,
            "phase": 19
          },
          {
            "value_slot": 40,
            "gate_slot": 51,
            "out_slot": 64,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 21,
            "out_slot": 65,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 22,
            "out_slot": 66,
            "phase": 19
          },
          {
            "value_slot": 48,
            "gate_slot": 29,
            "out_slot": 67,
            "phase": 19
          },
          {
            "value_slot": 31,
            "gate_slot": 29,
            "out_slot": 0,
            "phase": 23
          },
          {
            "value_slot": 28,
            "gate_slot": 13,
            "out_slot": 5,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 24,
            "out_slot": 10,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 47,
            "out_slot": 11,
            "phase": 23
          },
          {
            "value_slot": 15,
            "gate_slot": 25,
            "out_slot": 14,
            "phase": 23
          },
          {
            "value_slot": 16,
            "gate_slot": 27,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 46,
            "out_slot": 19,
            "phase": 23
          },
          {
            "value_slot": 20,
            "gate_slot": 53,
            "out_slot": 32,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 54,
            "out_slot": 33,
            "phase": 23
          },
          {
            "value_slot": 9,
            "gate_slot": 55,
            "out_slot": 34,
            "phase": 23
          },
          {
            "value_slot": 23,
            "gate_slot": 56,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 26,
            "gate_slot": 57,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 30,
            "gate_slot": 61,
            "out_slot": 37,
            "phase": 23
          },
          {
            "value_slot": 65,
            "gate_slot": 21,
            "out_slot": 38,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 21,
            "out_slot": 39,
            "phase": 23
          },
          {
            "value_slot": 66,
            "gate_slot": 22,
            "out_slot": 40,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 22,
            "out_slot": 41,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 67,
            "out_slot": 42,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 9,
            "value_slot": 8
          },
          {
            "channel": 2,
            "key_slot": 12,
            "value_slot": 39
          },
          {
            "channel": 3,
            "key_slot": 15,
            "value_slot": 41
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 6
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 30
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 7
          }
        ],
        "output_slots": [
          67,
          42,
          6,
          30,
          7
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
      "expectedTraceDigest": "0e6409beef97643fd301202571beb947a3f061f1167ade06fb98eb96805b0ae6",
      "expectedModelDigest": "36fe256c6e704b2b38718e52e1fd8e8b1ccf627f878d3062f2023fadd819bac3",
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
        "receipt.psionic.tassadar_graph.6ba49df561640d53",
        "receipt.psionic.tassadar_bundle.60eea91f657d636f",
        "receipt.psionic.tassadar_numeric_model.36fe256c6e704b2b",
        "receipt.psionic.tassadar_trace.0e6409beef97643f"
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
        "graph_digest": "39243b3f70f4fd60c23784d6222b5ad0bf1b7615061dbb40bda2ff720ba1094a",
        "bundle_digest": "76470bf8e52cf977a90d3b0189e4ac8deeefde246477a0dcc6d31c25463d69dd",
        "input_field_count": 1,
        "slot_count": 83,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 4
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 9,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 10,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 11,
            "bias": -3,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 13,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 14,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 15,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                7
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 35,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 36,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 37,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 38,
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
            "phase": 10
          },
          {
            "out_slot": 39,
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
            "phase": 10
          },
          {
            "out_slot": 40,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 41,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 42,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 43,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 44,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 45,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 46,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 47,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 48,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 49,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 50,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 51,
            "bias": -20,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 52,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 53,
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
            "out_slot": 54,
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
            "out_slot": 55,
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
            "out_slot": 56,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                59
              ],
              [
                -1,
                60
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
                61
              ],
              [
                -1,
                62
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                63
              ],
              [
                -1,
                64
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
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 24,
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
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                -1,
                52
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                79
              ],
              [
                -1,
                80
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                81
              ],
              [
                -1,
                82
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 29,
            "bias": 1,
            "terms": [
              [
                -1,
                28
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 32,
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
            "phase": 14
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
            "phase": 14
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                57
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                65
              ],
              [
                -1,
                66
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                67
              ],
              [
                -1,
                68
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 37,
            "bias": 0,
            "terms": [
              [
                1,
                69
              ],
              [
                -1,
                70
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 40,
            "bias": 0,
            "terms": [
              [
                1,
                71
              ],
              [
                -1,
                72
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
              [
                1,
                73
              ],
              [
                -1,
                74
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 42,
            "bias": 0,
            "terms": [
              [
                1,
                75
              ],
              [
                -1,
                76
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
              [
                1,
                77
              ],
              [
                -1,
                78
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 44,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 45,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                14
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 47,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 49,
            "bias": -1,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 50,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                1,
                27
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 51,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 9,
            "bias": 1,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                30
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                30
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                31
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                31
              ],
              [
                -1,
                32
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                33
              ],
              [
                -1,
                34
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 47,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                14
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 49,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                -1,
                36
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 51,
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
            "phase": 16
          },
          {
            "out_slot": 53,
            "bias": 0,
            "terms": [
              [
                1,
                37
              ],
              [
                -1,
                40
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 54,
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
            "phase": 16
          },
          {
            "out_slot": 55,
            "bias": 0,
            "terms": [
              [
                1,
                41
              ],
              [
                -1,
                42
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 56,
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
            "phase": 16
          },
          {
            "out_slot": 57,
            "bias": 0,
            "terms": [
              [
                1,
                43
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 58,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 9,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
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
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                47
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 31,
            "bias": -1,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 36,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 37,
            "bias": 1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 39,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 40,
            "bias": 1,
            "terms": [
              [
                -1,
                50
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                -1,
                15
              ],
              [
                -1,
                44
              ],
              [
                -1,
                45
              ],
              [
                -1,
                48
              ],
              [
                -1,
                49
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
                46
              ],
              [
                -1,
                53
              ],
              [
                -1,
                54
              ],
              [
                -1,
                55
              ],
              [
                -1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 42,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                4
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                1,
                25
              ],
              [
                1,
                26
              ],
              [
                1,
                27
              ],
              [
                1,
                46
              ],
              [
                1,
                51
              ],
              [
                1,
                53
              ],
              [
                1,
                54
              ],
              [
                1,
                55
              ],
              [
                1,
                56
              ],
              [
                1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                15
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
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
            "phase": 20
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                1,
                45
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                35
              ],
              [
                -1,
                0
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                0
              ],
              [
                -1,
                38
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 17,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                44
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
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                59
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 31,
            "bias": 1,
            "terms": [
              [
                1,
                60
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                0
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
                28
              ],
              [
                1,
                13
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                62
              ],
              [
                1,
                10
              ],
              [
                1,
                11
              ],
              [
                1,
                14
              ],
              [
                1,
                63
              ],
              [
                1,
                18
              ],
              [
                1,
                19
              ],
              [
                1,
                64
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ],
              [
                1,
                34
              ],
              [
                1,
                35
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 9,
            "bias": -1000,
            "terms": [
              [
                1,
                37
              ],
              [
                1000,
                61
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
                38
              ],
              [
                1000,
                21
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 15,
            "bias": -1000,
            "terms": [
              [
                1,
                40
              ],
              [
                1000,
                22
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
              "query_slot": 1,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 5,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 6,
              "out_slot": 7,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 5,
              "out_slot": 8,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 6,
              "out_slot": 10,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 28,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 9,
              "out_slot": 6,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 16,
              "out_slot": 7,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 17,
              "out_slot": 12,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 17,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 18,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 19,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 20,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 21,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 22,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 23,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 17,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 18,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 19,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 20,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 21,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 22,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 23,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 60,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 61,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 62,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 63,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 35,
            "out_slot": 64,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 65,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 66,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 40,
            "out_slot": 67,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 41,
            "out_slot": 68,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 42,
            "out_slot": 69,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 43,
            "out_slot": 70,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 44,
            "out_slot": 71,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 45,
            "out_slot": 72,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 73,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 74,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 75,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 76,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 50,
            "out_slot": 77,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 51,
            "out_slot": 78,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 53,
            "out_slot": 79,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 54,
            "out_slot": 80,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 55,
            "out_slot": 81,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 56,
            "out_slot": 82,
            "phase": 11
          },
          {
            "value_slot": 10,
            "gate_slot": 9,
            "out_slot": 11,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 17,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 18,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 19,
            "phase": 15
          },
          {
            "value_slot": 50,
            "gate_slot": 45,
            "out_slot": 20,
            "phase": 15
          },
          {
            "value_slot": 51,
            "gate_slot": 29,
            "out_slot": 21,
            "phase": 15
          },
          {
            "value_slot": 44,
            "gate_slot": 29,
            "out_slot": 22,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 7,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 15,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 13,
            "out_slot": 16,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 14,
            "out_slot": 17,
            "phase": 19
          },
          {
            "value_slot": 10,
            "gate_slot": 8,
            "out_slot": 18,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 19,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 44,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 45,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 49,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 50,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 39,
            "out_slot": 58,
            "phase": 19
          },
          {
            "value_slot": 41,
            "gate_slot": 29,
            "out_slot": 59,
            "phase": 19
          },
          {
            "value_slot": 42,
            "gate_slot": 20,
            "out_slot": 60,
            "phase": 19
          },
          {
            "value_slot": 43,
            "gate_slot": 29,
            "out_slot": 61,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 23,
            "out_slot": 62,
            "phase": 19
          },
          {
            "value_slot": 52,
            "gate_slot": 26,
            "out_slot": 63,
            "phase": 19
          },
          {
            "value_slot": 40,
            "gate_slot": 51,
            "out_slot": 64,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 21,
            "out_slot": 65,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 22,
            "out_slot": 66,
            "phase": 19
          },
          {
            "value_slot": 48,
            "gate_slot": 29,
            "out_slot": 67,
            "phase": 19
          },
          {
            "value_slot": 31,
            "gate_slot": 29,
            "out_slot": 0,
            "phase": 23
          },
          {
            "value_slot": 28,
            "gate_slot": 13,
            "out_slot": 5,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 24,
            "out_slot": 10,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 47,
            "out_slot": 11,
            "phase": 23
          },
          {
            "value_slot": 15,
            "gate_slot": 25,
            "out_slot": 14,
            "phase": 23
          },
          {
            "value_slot": 16,
            "gate_slot": 27,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 46,
            "out_slot": 19,
            "phase": 23
          },
          {
            "value_slot": 20,
            "gate_slot": 53,
            "out_slot": 32,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 54,
            "out_slot": 33,
            "phase": 23
          },
          {
            "value_slot": 9,
            "gate_slot": 55,
            "out_slot": 34,
            "phase": 23
          },
          {
            "value_slot": 23,
            "gate_slot": 56,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 26,
            "gate_slot": 57,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 30,
            "gate_slot": 61,
            "out_slot": 37,
            "phase": 23
          },
          {
            "value_slot": 65,
            "gate_slot": 21,
            "out_slot": 38,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 21,
            "out_slot": 39,
            "phase": 23
          },
          {
            "value_slot": 66,
            "gate_slot": 22,
            "out_slot": 40,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 22,
            "out_slot": 41,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 67,
            "out_slot": 42,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 9,
            "value_slot": 8
          },
          {
            "channel": 2,
            "key_slot": 12,
            "value_slot": 39
          },
          {
            "channel": 3,
            "key_slot": 15,
            "value_slot": 41
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 6
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 30
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 7
          }
        ],
        "output_slots": [
          67,
          42,
          6,
          30,
          7
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
      "expectedTraceDigest": "6676807cea1e1f2840ace3485f8b6e49cea3bd29ae1264b7c1214761caceebff",
      "expectedModelDigest": "929c5df178a15085e118d3e1707ae52caadeedd60be731b8ba70d128d77df2c4",
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
        "receipt.psionic.tassadar_graph.39243b3f70f4fd60",
        "receipt.psionic.tassadar_bundle.76470bf8e52cf977",
        "receipt.psionic.tassadar_numeric_model.929c5df178a15085",
        "receipt.psionic.tassadar_trace.6676807cea1e1f28"
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
        "graph_digest": "96d645801da2e7d85b1fef93a3307cbbaf9fc958cce781670286774665381c94",
        "bundle_digest": "5f967b3e470bf958a2c5fd7527f83aef49e411c9e26d66ab4d2d48b6d0607621",
        "input_field_count": 1,
        "slot_count": 83,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 4
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 9,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 10,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 11,
            "bias": -3,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 13,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 14,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 15,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                7
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 35,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 36,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 37,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 38,
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
            "phase": 10
          },
          {
            "out_slot": 39,
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
            "phase": 10
          },
          {
            "out_slot": 40,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 41,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 42,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 43,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 44,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 45,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 46,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 47,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 48,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 49,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 50,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 51,
            "bias": -20,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 52,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 53,
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
            "out_slot": 54,
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
            "out_slot": 55,
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
            "out_slot": 56,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                59
              ],
              [
                -1,
                60
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
                61
              ],
              [
                -1,
                62
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                63
              ],
              [
                -1,
                64
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
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 24,
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
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                -1,
                52
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                79
              ],
              [
                -1,
                80
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                81
              ],
              [
                -1,
                82
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 29,
            "bias": 1,
            "terms": [
              [
                -1,
                28
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 32,
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
            "phase": 14
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
            "phase": 14
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                57
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                65
              ],
              [
                -1,
                66
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                67
              ],
              [
                -1,
                68
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 37,
            "bias": 0,
            "terms": [
              [
                1,
                69
              ],
              [
                -1,
                70
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 40,
            "bias": 0,
            "terms": [
              [
                1,
                71
              ],
              [
                -1,
                72
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
              [
                1,
                73
              ],
              [
                -1,
                74
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 42,
            "bias": 0,
            "terms": [
              [
                1,
                75
              ],
              [
                -1,
                76
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
              [
                1,
                77
              ],
              [
                -1,
                78
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 44,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 45,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                14
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 47,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 49,
            "bias": -1,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 50,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                1,
                27
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 51,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 9,
            "bias": 1,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                30
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                30
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                31
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                31
              ],
              [
                -1,
                32
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                33
              ],
              [
                -1,
                34
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 47,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                14
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 49,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                -1,
                36
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 51,
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
            "phase": 16
          },
          {
            "out_slot": 53,
            "bias": 0,
            "terms": [
              [
                1,
                37
              ],
              [
                -1,
                40
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 54,
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
            "phase": 16
          },
          {
            "out_slot": 55,
            "bias": 0,
            "terms": [
              [
                1,
                41
              ],
              [
                -1,
                42
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 56,
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
            "phase": 16
          },
          {
            "out_slot": 57,
            "bias": 0,
            "terms": [
              [
                1,
                43
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 58,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 9,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
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
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                47
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 31,
            "bias": -1,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 36,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 37,
            "bias": 1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 39,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 40,
            "bias": 1,
            "terms": [
              [
                -1,
                50
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                -1,
                15
              ],
              [
                -1,
                44
              ],
              [
                -1,
                45
              ],
              [
                -1,
                48
              ],
              [
                -1,
                49
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
                46
              ],
              [
                -1,
                53
              ],
              [
                -1,
                54
              ],
              [
                -1,
                55
              ],
              [
                -1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 42,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                4
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                1,
                25
              ],
              [
                1,
                26
              ],
              [
                1,
                27
              ],
              [
                1,
                46
              ],
              [
                1,
                51
              ],
              [
                1,
                53
              ],
              [
                1,
                54
              ],
              [
                1,
                55
              ],
              [
                1,
                56
              ],
              [
                1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                15
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
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
            "phase": 20
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                1,
                45
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                35
              ],
              [
                -1,
                0
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                0
              ],
              [
                -1,
                38
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 17,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                44
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
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                59
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 31,
            "bias": 1,
            "terms": [
              [
                1,
                60
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                0
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
                28
              ],
              [
                1,
                13
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                62
              ],
              [
                1,
                10
              ],
              [
                1,
                11
              ],
              [
                1,
                14
              ],
              [
                1,
                63
              ],
              [
                1,
                18
              ],
              [
                1,
                19
              ],
              [
                1,
                64
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ],
              [
                1,
                34
              ],
              [
                1,
                35
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 9,
            "bias": -1000,
            "terms": [
              [
                1,
                37
              ],
              [
                1000,
                61
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
                38
              ],
              [
                1000,
                21
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 15,
            "bias": -1000,
            "terms": [
              [
                1,
                40
              ],
              [
                1000,
                22
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
              "query_slot": 1,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 5,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 6,
              "out_slot": 7,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 5,
              "out_slot": 8,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 6,
              "out_slot": 10,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 28,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 9,
              "out_slot": 6,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 16,
              "out_slot": 7,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 17,
              "out_slot": 12,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 17,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 18,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 19,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 20,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 21,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 22,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 23,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 17,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 18,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 19,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 20,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 21,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 22,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 23,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 60,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 61,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 62,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 63,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 35,
            "out_slot": 64,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 65,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 66,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 40,
            "out_slot": 67,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 41,
            "out_slot": 68,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 42,
            "out_slot": 69,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 43,
            "out_slot": 70,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 44,
            "out_slot": 71,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 45,
            "out_slot": 72,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 73,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 74,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 75,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 76,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 50,
            "out_slot": 77,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 51,
            "out_slot": 78,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 53,
            "out_slot": 79,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 54,
            "out_slot": 80,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 55,
            "out_slot": 81,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 56,
            "out_slot": 82,
            "phase": 11
          },
          {
            "value_slot": 10,
            "gate_slot": 9,
            "out_slot": 11,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 17,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 18,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 19,
            "phase": 15
          },
          {
            "value_slot": 50,
            "gate_slot": 45,
            "out_slot": 20,
            "phase": 15
          },
          {
            "value_slot": 51,
            "gate_slot": 29,
            "out_slot": 21,
            "phase": 15
          },
          {
            "value_slot": 44,
            "gate_slot": 29,
            "out_slot": 22,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 7,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 15,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 13,
            "out_slot": 16,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 14,
            "out_slot": 17,
            "phase": 19
          },
          {
            "value_slot": 10,
            "gate_slot": 8,
            "out_slot": 18,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 19,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 44,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 45,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 49,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 50,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 39,
            "out_slot": 58,
            "phase": 19
          },
          {
            "value_slot": 41,
            "gate_slot": 29,
            "out_slot": 59,
            "phase": 19
          },
          {
            "value_slot": 42,
            "gate_slot": 20,
            "out_slot": 60,
            "phase": 19
          },
          {
            "value_slot": 43,
            "gate_slot": 29,
            "out_slot": 61,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 23,
            "out_slot": 62,
            "phase": 19
          },
          {
            "value_slot": 52,
            "gate_slot": 26,
            "out_slot": 63,
            "phase": 19
          },
          {
            "value_slot": 40,
            "gate_slot": 51,
            "out_slot": 64,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 21,
            "out_slot": 65,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 22,
            "out_slot": 66,
            "phase": 19
          },
          {
            "value_slot": 48,
            "gate_slot": 29,
            "out_slot": 67,
            "phase": 19
          },
          {
            "value_slot": 31,
            "gate_slot": 29,
            "out_slot": 0,
            "phase": 23
          },
          {
            "value_slot": 28,
            "gate_slot": 13,
            "out_slot": 5,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 24,
            "out_slot": 10,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 47,
            "out_slot": 11,
            "phase": 23
          },
          {
            "value_slot": 15,
            "gate_slot": 25,
            "out_slot": 14,
            "phase": 23
          },
          {
            "value_slot": 16,
            "gate_slot": 27,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 46,
            "out_slot": 19,
            "phase": 23
          },
          {
            "value_slot": 20,
            "gate_slot": 53,
            "out_slot": 32,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 54,
            "out_slot": 33,
            "phase": 23
          },
          {
            "value_slot": 9,
            "gate_slot": 55,
            "out_slot": 34,
            "phase": 23
          },
          {
            "value_slot": 23,
            "gate_slot": 56,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 26,
            "gate_slot": 57,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 30,
            "gate_slot": 61,
            "out_slot": 37,
            "phase": 23
          },
          {
            "value_slot": 65,
            "gate_slot": 21,
            "out_slot": 38,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 21,
            "out_slot": 39,
            "phase": 23
          },
          {
            "value_slot": 66,
            "gate_slot": 22,
            "out_slot": 40,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 22,
            "out_slot": 41,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 67,
            "out_slot": 42,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 9,
            "value_slot": 8
          },
          {
            "channel": 2,
            "key_slot": 12,
            "value_slot": 39
          },
          {
            "channel": 3,
            "key_slot": 15,
            "value_slot": 41
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 6
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 30
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 7
          }
        ],
        "output_slots": [
          67,
          42,
          6,
          30,
          7
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
      "expectedTraceDigest": "99a8795d351ea495ada90289e2522af2ca2ae846496788cf051c7a3aa3de5801",
      "expectedModelDigest": "f16b8b0a2ffe253cbfcd35af817a05053e18a230b8ebce6b9b918fedd73703e8",
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
        "receipt.psionic.tassadar_graph.96d645801da2e7d8",
        "receipt.psionic.tassadar_bundle.5f967b3e470bf958",
        "receipt.psionic.tassadar_numeric_model.f16b8b0a2ffe253c",
        "receipt.psionic.tassadar_trace.99a8795d351ea495"
      ]
    },
    {
      "fixtureId": "tassadar_corpus.w1_1_window_v1.numeric_fixture.v1",
      "programId": "tassadar_corpus.w1_1_window_v1",
      "programDigest": "0b0804543211139b617fddcea492cc2330d25d81190ef1b2f1e8310274e85a8e",
      "workloadKind": "wasm_window.w1_1_stack_comparison_ladder",
      "profileId": "tassadar.wasm.core_i32_w1_1.v1",
      "program": {
        "program_id": "tassadar_corpus.w1_1_window_v1",
        "profile_id": "tassadar.wasm.core_i32_w1_1.v1",
        "local_count": 1,
        "memory_slots": 1,
        "initial_memory": [
          0
        ],
        "instructions": [
          {
            "opcode": "nop"
          },
          {
            "opcode": "i32_const",
            "value": 5
          },
          {
            "opcode": "local_tee",
            "local": 0
          },
          {
            "opcode": "drop"
          },
          {
            "opcode": "local_get",
            "local": 0
          },
          {
            "opcode": "i32_eqz"
          },
          {
            "opcode": "i32_const",
            "value": 0
          },
          {
            "opcode": "i32_eq"
          },
          {
            "opcode": "i32_const",
            "value": 7
          },
          {
            "opcode": "i32_const",
            "value": 3
          },
          {
            "opcode": "i32_gt"
          },
          {
            "opcode": "i32_add"
          },
          {
            "opcode": "i32_const",
            "value": 2
          },
          {
            "opcode": "i32_le"
          },
          {
            "opcode": "i32_const",
            "value": 1
          },
          {
            "opcode": "i32_ge"
          },
          {
            "opcode": "i32_const",
            "value": 0
          },
          {
            "opcode": "i32_ne"
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
        "model_id": "alm.numeric.tassadar.alm_wasm_interpreter.v1.tassadar_corpus.w1_1_window_v1",
        "graph_digest": "d48b8696fd824c868a5abb62302058f4cbc7456676d5782d21f9e60bf60bcf1f",
        "bundle_digest": "7f5aecff95ee9ec1aef997007f7c098e0b2bcd428031bac4daeee93706f71f19",
        "input_field_count": 1,
        "slot_count": 83,
        "layer_count": 6,
        "seed_writes": [
          [
            0,
            0,
            12
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
            13
          ],
          [
            0,
            5,
            0
          ],
          [
            0,
            6,
            14
          ],
          [
            0,
            7,
            0
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
            15
          ],
          [
            0,
            11,
            0
          ],
          [
            0,
            12,
            0
          ],
          [
            0,
            13,
            0
          ],
          [
            0,
            14,
            16
          ],
          [
            0,
            15,
            0
          ],
          [
            0,
            16,
            0
          ],
          [
            0,
            17,
            7
          ],
          [
            0,
            18,
            0
          ],
          [
            0,
            19,
            3
          ],
          [
            0,
            20,
            18
          ],
          [
            0,
            21,
            0
          ],
          [
            0,
            22,
            3
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
            2
          ],
          [
            0,
            26,
            19
          ],
          [
            0,
            27,
            0
          ],
          [
            0,
            28,
            0
          ],
          [
            0,
            29,
            1
          ],
          [
            0,
            30,
            20
          ],
          [
            0,
            31,
            0
          ],
          [
            0,
            32,
            0
          ],
          [
            0,
            33,
            0
          ],
          [
            0,
            34,
            17
          ],
          [
            0,
            35,
            0
          ],
          [
            0,
            36,
            10
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
            0,
            42,
            11
          ],
          [
            0,
            43,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 4
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 9,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 10,
            "bias": -2,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 11,
            "bias": -3,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 13,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 14,
            "bias": -13,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 15,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 6
          },
          {
            "out_slot": 6,
            "bias": -1,
            "terms": [
              [
                1,
                5
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 8
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                7
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 12,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
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
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 15,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 24,
            "bias": -4,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 25,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 26,
            "bias": -5,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 27,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 28,
            "bias": -6,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 29,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 30,
            "bias": -7,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 31,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 32,
            "bias": -8,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 33,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 34,
            "bias": -9,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 35,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 36,
            "bias": -10,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 37,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 38,
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
            "phase": 10
          },
          {
            "out_slot": 39,
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
            "phase": 10
          },
          {
            "out_slot": 40,
            "bias": -14,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 41,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 42,
            "bias": -15,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 43,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 44,
            "bias": -16,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 45,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 46,
            "bias": -17,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 47,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 48,
            "bias": -18,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 49,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 50,
            "bias": -19,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 51,
            "bias": -20,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 52,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                -1,
                8
              ]
            ],
            "input_field": null,
            "phase": 10
          },
          {
            "out_slot": 53,
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
            "out_slot": 54,
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
            "out_slot": 55,
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
            "out_slot": 56,
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
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                59
              ],
              [
                -1,
                60
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
                61
              ],
              [
                -1,
                62
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                63
              ],
              [
                -1,
                64
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
                12
              ],
              [
                -1,
                13
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 24,
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
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                -1,
                52
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                79
              ],
              [
                -1,
                80
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                81
              ],
              [
                -1,
                82
              ]
            ],
            "input_field": null,
            "phase": 12
          },
          {
            "out_slot": 29,
            "bias": 1,
            "terms": [
              [
                -1,
                28
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 31,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 32,
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
            "phase": 14
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
            "phase": 14
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                57
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 35,
            "bias": 0,
            "terms": [
              [
                1,
                65
              ],
              [
                -1,
                66
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 36,
            "bias": 0,
            "terms": [
              [
                1,
                67
              ],
              [
                -1,
                68
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 37,
            "bias": 0,
            "terms": [
              [
                1,
                69
              ],
              [
                -1,
                70
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 40,
            "bias": 0,
            "terms": [
              [
                1,
                71
              ],
              [
                -1,
                72
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
              [
                1,
                73
              ],
              [
                -1,
                74
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 42,
            "bias": 0,
            "terms": [
              [
                1,
                75
              ],
              [
                -1,
                76
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
              [
                1,
                77
              ],
              [
                -1,
                78
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 44,
            "bias": 0,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 45,
            "bias": 0,
            "terms": [
              [
                1,
                11
              ],
              [
                -1,
                14
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 47,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 49,
            "bias": -1,
            "terms": [
              [
                1,
                25
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 50,
            "bias": 0,
            "terms": [
              [
                1,
                26
              ],
              [
                1,
                27
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 51,
            "bias": 0,
            "terms": [
              [
                1,
                15
              ],
              [
                1,
                24
              ]
            ],
            "input_field": null,
            "phase": 14
          },
          {
            "out_slot": 9,
            "bias": 1,
            "terms": [
              [
                2,
                4
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                30
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 24,
            "bias": 0,
            "terms": [
              [
                1,
                30
              ],
              [
                -1,
                12
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 25,
            "bias": 0,
            "terms": [
              [
                1,
                13
              ],
              [
                -1,
                31
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                31
              ],
              [
                -1,
                32
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 27,
            "bias": 0,
            "terms": [
              [
                1,
                32
              ],
              [
                -1,
                33
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 46,
            "bias": 0,
            "terms": [
              [
                1,
                33
              ],
              [
                -1,
                34
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 47,
            "bias": 0,
            "terms": [
              [
                1,
                34
              ],
              [
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 48,
            "bias": 0,
            "terms": [
              [
                1,
                14
              ],
              [
                -1,
                35
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 49,
            "bias": 0,
            "terms": [
              [
                1,
                39
              ],
              [
                -1,
                36
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 51,
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
            "phase": 16
          },
          {
            "out_slot": 53,
            "bias": 0,
            "terms": [
              [
                1,
                37
              ],
              [
                -1,
                40
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 54,
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
            "phase": 16
          },
          {
            "out_slot": 55,
            "bias": 0,
            "terms": [
              [
                1,
                41
              ],
              [
                -1,
                42
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 56,
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
            "phase": 16
          },
          {
            "out_slot": 57,
            "bias": 0,
            "terms": [
              [
                1,
                43
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 58,
            "bias": 0,
            "terms": [
              [
                1,
                8
              ],
              [
                -1,
                10
              ]
            ],
            "input_field": null,
            "phase": 16
          },
          {
            "out_slot": 9,
            "bias": -11,
            "terms": [
              [
                1,
                7
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 12,
            "bias": -12,
            "terms": [
              [
                1,
                7
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
                24
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                47
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 31,
            "bias": -1,
            "terms": [
              [
                1,
                58
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 32,
            "bias": 0,
            "terms": [
              [
                1,
                16
              ],
              [
                -1,
                17
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 33,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                19
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 34,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 36,
            "bias": -1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 37,
            "bias": 1,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 39,
            "bias": 0,
            "terms": [
              [
                1,
                52
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 40,
            "bias": 1,
            "terms": [
              [
                -1,
                50
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 41,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                -1,
                15
              ],
              [
                -1,
                44
              ],
              [
                -1,
                45
              ],
              [
                -1,
                48
              ],
              [
                -1,
                49
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
                46
              ],
              [
                -1,
                53
              ],
              [
                -1,
                54
              ],
              [
                -1,
                55
              ],
              [
                -1,
                56
              ],
              [
                -1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 42,
            "bias": -1,
            "terms": [
              [
                1,
                6
              ],
              [
                -1,
                4
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 43,
            "bias": 0,
            "terms": [
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
                47
              ],
              [
                1,
                25
              ],
              [
                1,
                26
              ],
              [
                1,
                27
              ],
              [
                1,
                46
              ],
              [
                1,
                51
              ],
              [
                1,
                53
              ],
              [
                1,
                54
              ],
              [
                1,
                55
              ],
              [
                1,
                56
              ],
              [
                1,
                57
              ]
            ],
            "input_field": null,
            "phase": 18
          },
          {
            "out_slot": 0,
            "bias": 0,
            "terms": [
              [
                1,
                7
              ],
              [
                -1,
                15
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
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
            "phase": 20
          },
          {
            "out_slot": 9,
            "bias": 0,
            "terms": [
              [
                1,
                45
              ],
              [
                -1,
                49
              ]
            ],
            "input_field": null,
            "phase": 20
          },
          {
            "out_slot": 13,
            "bias": 0,
            "terms": [
              [
                1,
                35
              ],
              [
                -1,
                0
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 14,
            "bias": 0,
            "terms": [
              [
                1,
                0
              ],
              [
                -1,
                38
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 15,
            "bias": 0,
            "terms": [
              [
                1,
                10
              ],
              [
                1,
                8
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 16,
            "bias": 0,
            "terms": [
              [
                1,
                18
              ],
              [
                -1,
                11
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 17,
            "bias": 0,
            "terms": [
              [
                1,
                19
              ],
              [
                -1,
                44
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
                -1,
                6
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 23,
            "bias": 1,
            "terms": [
              [
                -1,
                9
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 26,
            "bias": 0,
            "terms": [
              [
                1,
                50
              ],
              [
                -1,
                58
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 30,
            "bias": 0,
            "terms": [
              [
                1,
                5
              ],
              [
                1,
                59
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 31,
            "bias": 1,
            "terms": [
              [
                1,
                60
              ]
            ],
            "input_field": null,
            "phase": 22
          },
          {
            "out_slot": 6,
            "bias": 0,
            "terms": [
              [
                1,
                4
              ],
              [
                1,
                0
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
                28
              ],
              [
                1,
                13
              ],
              [
                -1,
                5
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 8,
            "bias": 0,
            "terms": [
              [
                1,
                62
              ],
              [
                1,
                10
              ],
              [
                1,
                11
              ],
              [
                1,
                14
              ],
              [
                1,
                63
              ],
              [
                1,
                18
              ],
              [
                1,
                19
              ],
              [
                1,
                64
              ],
              [
                1,
                32
              ],
              [
                1,
                33
              ],
              [
                1,
                34
              ],
              [
                1,
                35
              ],
              [
                1,
                36
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 9,
            "bias": -1000,
            "terms": [
              [
                1,
                37
              ],
              [
                1000,
                61
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
                38
              ],
              [
                1000,
                21
              ]
            ],
            "input_field": null,
            "phase": 24
          },
          {
            "out_slot": 15,
            "bias": -1000,
            "terms": [
              [
                1,
                40
              ],
              [
                1000,
                22
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
              "query_slot": 1,
              "out_slot": 4,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 2,
              "out_slot": 5,
              "phase": 1
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 6,
              "out_slot": 7,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 5,
              "out_slot": 8,
              "phase": 5
            }
          },
          {
            "keyed_read": {
              "channel": 1,
              "query_slot": 6,
              "out_slot": 10,
              "phase": 9
            }
          },
          {
            "keyed_read": {
              "channel": 4,
              "query_slot": 3,
              "out_slot": 28,
              "phase": 13
            }
          },
          {
            "keyed_read": {
              "channel": 0,
              "query_slot": 9,
              "out_slot": 6,
              "phase": 17
            }
          },
          {
            "keyed_read": {
              "channel": 2,
              "query_slot": 16,
              "out_slot": 7,
              "phase": 21
            }
          },
          {
            "keyed_read": {
              "channel": 3,
              "query_slot": 17,
              "out_slot": 12,
              "phase": 21
            }
          }
        ],
        "ffn": [
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 17,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 10,
            "out_slot": 18,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 19,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 20,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 13,
            "out_slot": 21,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 22,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 23,
            "phase": 7
          },
          {
            "value_slot": 0,
            "gate_slot": 6,
            "out_slot": 16,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 11,
            "out_slot": 17,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 14,
            "out_slot": 18,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 15,
            "out_slot": 19,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 24,
            "out_slot": 20,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 25,
            "out_slot": 21,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 26,
            "out_slot": 22,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 27,
            "out_slot": 23,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 28,
            "out_slot": 57,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 29,
            "out_slot": 58,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 59,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 60,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 32,
            "out_slot": 61,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 33,
            "out_slot": 62,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 63,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 35,
            "out_slot": 64,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 65,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 66,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 40,
            "out_slot": 67,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 41,
            "out_slot": 68,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 42,
            "out_slot": 69,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 43,
            "out_slot": 70,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 44,
            "out_slot": 71,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 45,
            "out_slot": 72,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 73,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 74,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 75,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 76,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 50,
            "out_slot": 77,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 51,
            "out_slot": 78,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 53,
            "out_slot": 79,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 54,
            "out_slot": 80,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 55,
            "out_slot": 81,
            "phase": 11
          },
          {
            "value_slot": 0,
            "gate_slot": 56,
            "out_slot": 82,
            "phase": 11
          },
          {
            "value_slot": 10,
            "gate_slot": 9,
            "out_slot": 11,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 46,
            "out_slot": 16,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 47,
            "out_slot": 17,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 48,
            "out_slot": 18,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 49,
            "out_slot": 19,
            "phase": 15
          },
          {
            "value_slot": 50,
            "gate_slot": 45,
            "out_slot": 20,
            "phase": 15
          },
          {
            "value_slot": 51,
            "gate_slot": 29,
            "out_slot": 21,
            "phase": 15
          },
          {
            "value_slot": 44,
            "gate_slot": 29,
            "out_slot": 22,
            "phase": 15
          },
          {
            "value_slot": 0,
            "gate_slot": 9,
            "out_slot": 7,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 12,
            "out_slot": 15,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 13,
            "out_slot": 16,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 14,
            "out_slot": 17,
            "phase": 19
          },
          {
            "value_slot": 10,
            "gate_slot": 8,
            "out_slot": 18,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 30,
            "out_slot": 19,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 31,
            "out_slot": 44,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 34,
            "out_slot": 45,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 36,
            "out_slot": 49,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 37,
            "out_slot": 50,
            "phase": 19
          },
          {
            "value_slot": 0,
            "gate_slot": 39,
            "out_slot": 58,
            "phase": 19
          },
          {
            "value_slot": 41,
            "gate_slot": 29,
            "out_slot": 59,
            "phase": 19
          },
          {
            "value_slot": 42,
            "gate_slot": 20,
            "out_slot": 60,
            "phase": 19
          },
          {
            "value_slot": 43,
            "gate_slot": 29,
            "out_slot": 61,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 23,
            "out_slot": 62,
            "phase": 19
          },
          {
            "value_slot": 52,
            "gate_slot": 26,
            "out_slot": 63,
            "phase": 19
          },
          {
            "value_slot": 40,
            "gate_slot": 51,
            "out_slot": 64,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 21,
            "out_slot": 65,
            "phase": 19
          },
          {
            "value_slot": 6,
            "gate_slot": 22,
            "out_slot": 66,
            "phase": 19
          },
          {
            "value_slot": 48,
            "gate_slot": 29,
            "out_slot": 67,
            "phase": 19
          },
          {
            "value_slot": 31,
            "gate_slot": 29,
            "out_slot": 0,
            "phase": 23
          },
          {
            "value_slot": 28,
            "gate_slot": 13,
            "out_slot": 5,
            "phase": 23
          },
          {
            "value_slot": 7,
            "gate_slot": 24,
            "out_slot": 10,
            "phase": 23
          },
          {
            "value_slot": 12,
            "gate_slot": 47,
            "out_slot": 11,
            "phase": 23
          },
          {
            "value_slot": 15,
            "gate_slot": 25,
            "out_slot": 14,
            "phase": 23
          },
          {
            "value_slot": 16,
            "gate_slot": 27,
            "out_slot": 18,
            "phase": 23
          },
          {
            "value_slot": 17,
            "gate_slot": 46,
            "out_slot": 19,
            "phase": 23
          },
          {
            "value_slot": 20,
            "gate_slot": 53,
            "out_slot": 32,
            "phase": 23
          },
          {
            "value_slot": 6,
            "gate_slot": 54,
            "out_slot": 33,
            "phase": 23
          },
          {
            "value_slot": 9,
            "gate_slot": 55,
            "out_slot": 34,
            "phase": 23
          },
          {
            "value_slot": 23,
            "gate_slot": 56,
            "out_slot": 35,
            "phase": 23
          },
          {
            "value_slot": 26,
            "gate_slot": 57,
            "out_slot": 36,
            "phase": 23
          },
          {
            "value_slot": 30,
            "gate_slot": 61,
            "out_slot": 37,
            "phase": 23
          },
          {
            "value_slot": 65,
            "gate_slot": 21,
            "out_slot": 38,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 21,
            "out_slot": 39,
            "phase": 23
          },
          {
            "value_slot": 66,
            "gate_slot": 22,
            "out_slot": 40,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 22,
            "out_slot": 41,
            "phase": 23
          },
          {
            "value_slot": 8,
            "gate_slot": 67,
            "out_slot": 42,
            "phase": 23
          }
        ],
        "writes": [
          {
            "channel": 1,
            "key_slot": 9,
            "value_slot": 8
          },
          {
            "channel": 2,
            "key_slot": 12,
            "value_slot": 39
          },
          {
            "channel": 3,
            "key_slot": 15,
            "value_slot": 41
          },
          {
            "channel": 4,
            "key_slot": 1,
            "value_slot": 6
          },
          {
            "channel": 4,
            "key_slot": 2,
            "value_slot": 30
          },
          {
            "channel": 4,
            "key_slot": 3,
            "value_slot": 7
          }
        ],
        "output_slots": [
          67,
          42,
          6,
          30,
          7
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
        ]
      ],
      "expectedTraceDigest": "d472c32fa0d4b651cfc4e29c6203dcc6211253f7d11654d63f0de5314d46ffb3",
      "expectedModelDigest": "caf66939e14b7d4fb823c80c9f8969f7604407b6fabf5b7e6a895f1b54b9556e",
      "expectedFinalRow": [
        0,
        0,
        20,
        0,
        1
      ],
      "expectedOutputs": [
        1
      ],
      "halted": true,
      "compileReceiptRefs": [
        "receipt.psionic.tassadar_program.0b0804543211139b",
        "receipt.psionic.tassadar_graph.d48b8696fd824c86",
        "receipt.psionic.tassadar_bundle.7f5aecff95ee9ec1",
        "receipt.psionic.tassadar_numeric_model.caf66939e14b7d4f",
        "receipt.psionic.tassadar_trace.d472c32fa0d4b651"
      ]
    }
  ],
  "corpusDigest": "0d347bc3081acd2740761673f0b70d3e17a5ae467e9f865b5e6ef12009bfeb49"
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
