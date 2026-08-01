import { realpathSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

const isWithin = (path: string, root: string): boolean =>
  path === root || path.startsWith(`${root}/`);

export const canonicalFailureMatrixPaths = (
  repositoryRoot: string,
  inputPath: string,
  receiptPath: string,
): Readonly<{ inputPath: string; receiptPath: string }> => {
  const lexicalRepositoryRoot = resolve(repositoryRoot);
  const canonicalRepositoryRoot = realpathSync(repositoryRoot);
  const canonicalReceiptRoot = realpathSync(
    resolve(canonicalRepositoryRoot, "docs/ops/receipts/livekit"),
  );
  const lexicalInputPath = resolve(inputPath);
  const canonicalInputPath = realpathSync(lexicalInputPath);
  if (
    isWithin(lexicalInputPath, lexicalRepositoryRoot) ||
    isWithin(canonicalInputPath, canonicalRepositoryRoot)
  ) {
    throw new Error("private failure-matrix observation must remain outside the repository");
  }

  const lexicalReceiptPath = resolve(canonicalRepositoryRoot, receiptPath);
  const lexicalReceiptRoot = resolve(canonicalRepositoryRoot, "docs/ops/receipts/livekit");
  if (!lexicalReceiptPath.startsWith(`${lexicalReceiptRoot}/`)) {
    throw new Error("receipt path must be under docs/ops/receipts/livekit");
  }
  const canonicalReceiptPath = join(
    realpathSync(dirname(lexicalReceiptPath)),
    basename(lexicalReceiptPath),
  );
  if (!canonicalReceiptPath.startsWith(`${canonicalReceiptRoot}/`)) {
    throw new Error("receipt path must be under docs/ops/receipts/livekit");
  }
  return { inputPath: canonicalInputPath, receiptPath: canonicalReceiptPath };
};
