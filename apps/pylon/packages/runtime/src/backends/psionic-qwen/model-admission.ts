import {
  PSIONIC_QWEN_KNOWN_ARTIFACT_DIGESTS,
  PSIONIC_QWEN_MODEL_REFS,
  type PsionicQwenModelListResponse,
} from "./contract.js";

export type PsionicQwenModelRef = typeof PSIONIC_QWEN_MODEL_REFS[keyof typeof PSIONIC_QWEN_MODEL_REFS];
export type PsionicQwenModelRowKey = keyof typeof PSIONIC_QWEN_MODEL_REFS;
export type PsionicQwenTaskMode = "install_smoke" | "health_probe" | "simple_local_answer" | "fallback" | "coding_agent" | "requires_2b";

export interface PsionicQwenModelDescriptor {
  readonly id: string;
  readonly artifactDigest?: string;
  readonly artifactManifestRef?: string;
}

export interface PsionicQwenAdmittedModelRow {
  readonly rowKey: PsionicQwenModelRowKey;
  readonly modelRef: PsionicQwenModelRef;
  readonly artifactDigest?: string;
  readonly artifactManifestRef?: string;
  readonly verificationRef: string;
}

export interface PsionicQwenModelAdmission {
  readonly rows: ReadonlyArray<PsionicQwenAdmittedModelRow>;
  readonly admittedModelRefs: ReadonlyArray<PsionicQwenModelRef>;
  readonly observedModelRefs: ReadonlyArray<PsionicQwenModelRef>;
  readonly blockerRefs: ReadonlyArray<string>;
}

export interface PsionicQwenModelSelection {
  readonly admitted: boolean;
  readonly mode: PsionicQwenTaskMode;
  readonly selectedModelRef: PsionicQwenModelRef | null;
  readonly blockerRefs: ReadonlyArray<string>;
}

export function admitPsionicQwenModelRows(
  descriptors: ReadonlyArray<PsionicQwenModelDescriptor>,
): PsionicQwenModelAdmission {
  const observedModelRefs = uniqueRefs(descriptors.flatMap((descriptor) => modelRefFromModelId(descriptor.id)));
  const rows: PsionicQwenAdmittedModelRow[] = [];
  const blockerRefs = new Set<string>();

  for (const descriptor of descriptors) {
    const rowKey = modelRowKeyFromModelId(descriptor.id);

    if (rowKey === undefined) {
      continue;
    }

    const verificationRef = verificationRefForDescriptor(rowKey, descriptor);

    if (verificationRef === undefined) {
      blockerRefs.add("blocker.psionic_qwen35.artifact_digest_unverified");
      continue;
    }

    rows.push({
      rowKey,
      modelRef: PSIONIC_QWEN_MODEL_REFS[rowKey],
      artifactDigest: descriptor.artifactDigest,
      artifactManifestRef: descriptor.artifactManifestRef,
      verificationRef,
    });
  }

  if (observedModelRefs.length > 0 && rows.length === 0) {
    blockerRefs.add("blocker.psionic_qwen35.qwen35_model_missing");
  }

  return {
    rows,
    admittedModelRefs: uniqueRefs(rows.map((row) => row.modelRef)),
    observedModelRefs,
    blockerRefs: [...blockerRefs],
  };
}

export function selectPsionicQwenModel(
  admission: PsionicQwenModelAdmission,
  mode: PsionicQwenTaskMode,
): PsionicQwenModelSelection {
  const has08b = admission.admittedModelRefs.includes(PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b);
  const has2b = admission.admittedModelRefs.includes(PSIONIC_QWEN_MODEL_REFS.qwen35_2b);

  if (mode === "requires_2b") {
    return has2b
      ? admitted(mode, PSIONIC_QWEN_MODEL_REFS.qwen35_2b)
      : blocked(mode, "blocker.psionic_qwen35.model_2b_missing");
  }

  if (mode === "coding_agent") {
    if (has2b) {
      return admitted(mode, PSIONIC_QWEN_MODEL_REFS.qwen35_2b);
    }

    if (has08b) {
      return admitted(mode, PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b);
    }

    return blocked(mode, "blocker.psionic_qwen35.qwen35_model_missing");
  }

  if (has08b) {
    return admitted(mode, PSIONIC_QWEN_MODEL_REFS.qwen35_0_8b);
  }

  if (has2b) {
    return admitted(mode, PSIONIC_QWEN_MODEL_REFS.qwen35_2b);
  }

  return blocked(mode, mode === "install_smoke" ? "blocker.psionic_qwen35.model_0_8b_missing" : "blocker.psionic_qwen35.qwen35_model_missing");
}

export function descriptorsFromPsionicModelList(
  models: PsionicQwenModelListResponse,
): ReadonlyArray<PsionicQwenModelDescriptor> {
  return (models.data ?? []).flatMap((model) => {
    if (typeof model === "string") {
      return [{ id: model }];
    }

    const metadata = model.metadata;
    return [
      {
        id: model.id,
        artifactDigest: firstString(
          model.artifact_digest,
          model.artifactDigest,
          readMetadataString(metadata, "artifact_digest"),
          readMetadataString(metadata, "artifactDigest"),
          readMetadataString(metadata, "sha256"),
        ),
        artifactManifestRef: firstString(
          model.artifact_manifest_ref,
          model.artifactManifestRef,
          readMetadataString(metadata, "artifact_manifest_ref"),
          readMetadataString(metadata, "artifactManifestRef"),
          readMetadataString(metadata, "manifest_ref"),
        ),
      },
    ];
  });
}

export function modelRefFromModelId(modelId: string): ReadonlyArray<PsionicQwenModelRef> {
  const rowKey = modelRowKeyFromModelId(modelId);
  return rowKey === undefined ? [] : [PSIONIC_QWEN_MODEL_REFS[rowKey]];
}

function modelRowKeyFromModelId(modelId: string): PsionicQwenModelRowKey | undefined {
  const normalized = modelId.toLowerCase().replace(/[-/.:]+/g, "_");
  const isQwen35 = normalized.includes("qwen3_5") || normalized.includes("qwen35") || normalized.includes("qwen_3_5");

  if (!isQwen35) {
    return undefined;
  }

  if (normalized.includes("0_8") || normalized.includes("08b")) {
    return "qwen35_0_8b";
  }

  if (normalized.includes("2b") || normalized.includes("2_b")) {
    return "qwen35_2b";
  }

  return undefined;
}

function verificationRefForDescriptor(
  rowKey: PsionicQwenModelRowKey,
  descriptor: PsionicQwenModelDescriptor,
): string | undefined {
  if (descriptor.artifactManifestRef !== undefined && isPublicSafeRef(descriptor.artifactManifestRef)) {
    return descriptor.artifactManifestRef;
  }

  const expectedDigest = PSIONIC_QWEN_KNOWN_ARTIFACT_DIGESTS[rowKey];
  const digest = descriptor.artifactDigest?.toLowerCase();

  if (expectedDigest !== undefined && digest === expectedDigest) {
    return `artifact.digest.sha256.${digest}`;
  }

  return undefined;
}

function admitted(mode: PsionicQwenTaskMode, selectedModelRef: PsionicQwenModelRef): PsionicQwenModelSelection {
  return {
    admitted: true,
    mode,
    selectedModelRef,
    blockerRefs: [],
  };
}

function blocked(mode: PsionicQwenTaskMode, blockerRef: string): PsionicQwenModelSelection {
  return {
    admitted: false,
    mode,
    selectedModelRef: null,
    blockerRefs: [blockerRef],
  };
}

function isPublicSafeRef(value: string): boolean {
  return /^[a-z][a-z0-9._-]+$/.test(value);
}

function readMetadataString(metadata: Readonly<Record<string, unknown>> | undefined, key: string): string | undefined {
  const value = metadata?.[key];
  return typeof value === "string" ? value : undefined;
}

function firstString(...values: ReadonlyArray<string | undefined>): string | undefined {
  return values.find((value) => value !== undefined && value.trim().length > 0);
}

function uniqueRefs<T extends string>(values: ReadonlyArray<T>): ReadonlyArray<T> {
  return [...new Set(values)];
}
