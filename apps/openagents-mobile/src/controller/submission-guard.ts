export type SubmissionPhase = "idle" | "submitting";

export const createSubmissionGuard = () => {
  let phase: SubmissionPhase = "idle";
  return {
    phase: (): SubmissionPhase => phase,
    run: async <T>(
      task: () => Promise<T>,
      onPhase?: (phase: SubmissionPhase) => void,
    ): Promise<T | undefined> => {
      if (phase !== "idle") return undefined;
      phase = "submitting";
      onPhase?.(phase);
      try {
        return await task();
      } finally {
        phase = "idle";
        onPhase?.(phase);
      }
    },
  };
};
