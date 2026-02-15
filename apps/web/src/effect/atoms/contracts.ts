import { Atom, Result } from '@effect-atom/atom';
import { Cause, Effect } from 'effect';
import { ContractsApiService } from "../contracts";
import { AppAtomRuntime } from './appRuntime';

import type {
  ModuleItem,
  ModulesPageData,
  SignatureItem,
  SignaturesPageData,
  ToolItem,
  ToolsPageData,
} from "../../lib/pageData/contracts";
import type { ToolContract, ModuleContract, SignatureContract } from "../contracts";

function safeStableStringify(value: unknown, indent = 2): string {
  if (value == null) return String(value);
  if (typeof value === 'string') return value;
  try {
    return JSON.stringify(value, null, indent);
  } catch {
    return String(value);
  }
}

function summarizePromptIr(promptIr: unknown): string {
  if (!promptIr || typeof promptIr !== 'object') return '(missing prompt IR)';
  const obj = promptIr as { blocks?: Array<unknown> };
  const blocks = obj.blocks;
  if (!Array.isArray(blocks)) return '(missing blocks)';
  const tags = blocks.map((b) => (b && typeof b === 'object' ? String((b as { _tag?: unknown })._tag ?? '?') : '?'));
  return tags.join(' -> ');
}

function errorTextFromResult<TValue, TError>(
  result: Result.Result<TValue, TError>,
  fallback: string,
): string | null {
  if (Result.isFailure(result)) {
    const pretty = Cause.pretty(result.cause as Cause.Cause<unknown>);
    return pretty.trim() ? pretty : fallback;
  }
  return null;
}

export const ModuleContractsAtom = Atom.family((userId: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      void userId;
      const api = yield* ContractsApiService;
      return yield* api.getModuleContracts();
    }),
  ).pipe(Atom.keepAlive, Atom.withLabel(`ModuleContractsAtom(${userId})`)),
);

export const ToolContractsAtom = Atom.family((userId: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      void userId;
      const api = yield* ContractsApiService;
      return yield* api.getToolContracts();
    }),
  ).pipe(Atom.keepAlive, Atom.withLabel(`ToolContractsAtom(${userId})`)),
);

export const SignatureContractsAtom = Atom.family((userId: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      void userId;
      const api = yield* ContractsApiService;
      return yield* api.getSignatureContracts();
    }),
  ).pipe(Atom.keepAlive, Atom.withLabel(`SignatureContractsAtom(${userId})`)),
);

export const ModulesPageDataAtom = Atom.family((userId: string) =>
  Atom.make((get) => {
    const result = get(ModuleContractsAtom(userId));
    const errorText = errorTextFromResult(result, 'Failed to load module contracts.');
    if (errorText) return { errorText, sorted: null } satisfies ModulesPageData;

    if (Result.isSuccess(result)) {
      const mods = result.value as ReadonlyArray<ModuleContract>;
      const sorted = [...mods]
        .sort((a, b) => a.moduleId.localeCompare(b.moduleId))
        .map(
          (m): ModuleItem => ({
            moduleId: m.moduleId,
            description: m.description,
            signatureIdsJson: safeStableStringify(m.signatureIds),
          }),
        );
      return { errorText: null, sorted } satisfies ModulesPageData;
    }

    return { errorText: null, sorted: null } satisfies ModulesPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`ModulesPageDataAtom(${userId})`)),
);

export const ToolsPageDataAtom = Atom.family((userId: string) =>
  Atom.make((get) => {
    const result = get(ToolContractsAtom(userId));
    const errorText = errorTextFromResult(result, 'Failed to load tool contracts.');
    if (errorText) return { errorText, sorted: null } satisfies ToolsPageData;

    if (Result.isSuccess(result)) {
      const tools = result.value as ReadonlyArray<ToolContract>;
      const sorted = [...tools]
        .sort((a, b) => a.name.localeCompare(b.name))
        .map(
          (t): ToolItem => ({
            name: t.name,
            description: t.description,
            usage: t.usage ?? null,
            inputSchemaJson: safeStableStringify(t.inputSchemaJson),
            outputSchemaJson: safeStableStringify(t.outputSchemaJson ?? null),
          }),
        );
      return { errorText: null, sorted } satisfies ToolsPageData;
    }

    return { errorText: null, sorted: null } satisfies ToolsPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`ToolsPageDataAtom(${userId})`)),
);

export const SignaturesPageDataAtom = Atom.family((userId: string) =>
  Atom.make((get) => {
    const result = get(SignatureContractsAtom(userId));
    const errorText = errorTextFromResult(result, 'Failed to load signature contracts.');
    if (errorText) return { errorText, sorted: null } satisfies SignaturesPageData;

    if (Result.isSuccess(result)) {
      const sigs = result.value as ReadonlyArray<SignatureContract>;
      const sorted = [...sigs]
        .sort((a, b) => a.signatureId.localeCompare(b.signatureId))
        .map(
          (s): SignatureItem => ({
            signatureId: s.signatureId,
            promptSummary: summarizePromptIr(s.promptIr),
            inputSchemaJson: safeStableStringify(s.inputSchemaJson),
            outputSchemaJson: safeStableStringify(s.outputSchemaJson),
            promptIrJson: safeStableStringify(s.promptIr),
            defaultsJson: safeStableStringify({
              defaultParams: s.defaultParams,
              defaultConstraints: s.defaultConstraints,
            }),
          }),
        );
      return { errorText: null, sorted } satisfies SignaturesPageData;
    }

    return { errorText: null, sorted: null } satisfies SignaturesPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`SignaturesPageDataAtom(${userId})`)),
);
