export interface PerformanceSuiteInput {
  role: string;
  report?: unknown;
  evidenceVerified?: boolean;
  reference?: unknown;
  evidenceErrors?: unknown[];
  applicationEvidenceVerified?: boolean;
  applicationEvidenceErrors?: unknown[];
}
export interface PerformanceSuiteResult {
  schema: "aethyrforge.performance-suite.v1";
  ok: boolean;
  allProductQualified: boolean;
  presentationMs: null;
  results: Array<{
    role: string;
    ok: boolean;
    available: boolean;
    missing: Array<{
      permutation: string;
      viewport: string;
      repeats?: number;
      requiredRepeats?: number;
    }>;
    [key: string]: unknown;
  }>;
  limitations: string[];
  duplicates: boolean;
  application: {
    schema: "aethyrforge.application-suite.v1";
    qualified: boolean;
    compositorPresentation: null;
    physicalPresentation: null;
    results: Array<{
      role: string;
      qualified: boolean;
      available: boolean;
      missing: Array<{
        permutation: string;
        viewport: string;
        repeats?: number;
        requiredRepeats?: number;
      }>;
      evidenceErrors: unknown[];
      scope?: string;
    }>;
    limitations: string[];
  };
}
export function indexPerformanceSuite(
  inputs: readonly PerformanceSuiteInput[],
): PerformanceSuiteResult;

export function verifyApplicationWindow(
  row: any,
  raw: any,
  browserEvidence: unknown,
): any;

export function verifyPerformanceInput(
  role: string,
  path: string,
): Promise<PerformanceSuiteInput>;
