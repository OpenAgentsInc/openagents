import { Schema as S } from "effect"

import { canonicalArtifact } from "./artifact.ts"
import { Digest, NonEmptyString, PositiveInteger, RelativePath, StableRef } from "./schema.ts"
import { sha256Digest } from "./tooling.ts"

export const ASSURANCE_ADMISSION_FORMAT_VERSION = "0.1" as const

export const AssuranceAdmissionSchema = S.Struct({
  admission_format_version: S.Literal(ASSURANCE_ADMISSION_FORMAT_VERSION),
  admission_ref: StableRef,
  decision: S.Literal("admitted"),
  assurance_spec: S.Struct({
    id: StableRef,
    revision: PositiveInteger,
    document_digest: Digest,
  }),
  product_spec: S.Struct({
    path: RelativePath,
    revision: PositiveInteger,
    document_digest: Digest,
  }),
  review_set_digest: Digest,
  recognized_actor_ref: StableRef,
  recognized_role: StableRef,
  allowed_gate_refs: S.Array(StableRef),
  authority_statement: NonEmptyString,
})
export type AssuranceAdmission = typeof AssuranceAdmissionSchema.Type

export const decodeAssuranceAdmission = S.decodeUnknownSync(AssuranceAdmissionSchema)

export const assuranceReviewSetDigest = (
  reviewArtifacts: ReadonlyArray<Readonly<{ path: string; bytes: string }>>,
): `sha256:${string}` => canonicalArtifact(
  [...reviewArtifacts]
    .sort((left, right) => left.path.localeCompare(right.path))
    .map((artifact) => ({ path: artifact.path, digest: sha256Digest(artifact.bytes) })),
).digest
