/**
 * HillClimber Task Decomposer Module
 *
 * Breaks complex tasks into subtasks with verification checkpoints.
 * Task-specific decomposition rules for all Terminal-Bench 2 tasks.
 *
 * Part of the MAP-inspired architecture for 10x better HillClimber.
 */

import type { TerminalBenchTask } from "../bench/terminal-bench.js";

// ============================================================================
// Types
// ============================================================================

export interface Subtask {
  /** Unique subtask ID within the task */
  id: number;
  /** Short name for the subtask */
  name: string;
  /** Detailed goal description */
  goal: string;
  /** Verification checkpoint (what to check after completing) */
  checkpoint: string;
  /** Expected output files or artifacts */
  expectedArtifacts: string[];
  /** Dependencies on previous subtasks */
  dependsOn: number[];
  /** Hints specific to this subtask */
  hints: string[];
  /** Maximum turns to spend on this subtask */
  maxTurns: number;
}

export interface TaskDecomposition {
  /** Original task ID */
  taskId: string;
  /** Total number of subtasks */
  subtaskCount: number;
  /** Ordered list of subtasks */
  subtasks: Subtask[];
  /** Overall task hints (apply to all subtasks) */
  globalHints: string[];
  /** Files to read before starting */
  filesToRead: string[];
  /** Output files that must exist for success */
  requiredOutputs: string[];
}

// ============================================================================
// Task-Specific Decompositions
// ============================================================================

const REGEX_LOG_DECOMPOSITION: TaskDecomposition = {
  taskId: "regex-log",
  subtaskCount: 4,
  subtasks: [
    {
      id: 1,
      name: "understand-task",
      goal: "Read the task description and understand what regex patterns are needed",
      checkpoint: "Can explain: 1) What makes a valid IPv4, 2) What makes a valid date, 3) The 'last date' requirement",
      expectedArtifacts: [],
      dependsOn: [],
      hints: [
        "Valid IPv4: 0-255 per octet, no alphanumeric directly before/after",
        "Valid date: YYYY-MM-DD with month 01-12, day depends on month",
        "Match LAST date means use greedy .* before the date pattern",
      ],
      maxTurns: 3,
    },
    {
      id: 2,
      name: "write-initial-regex",
      goal: "Write an initial regex attempt to /app/regex.txt",
      checkpoint: "File /app/regex.txt exists with a regex pattern",
      expectedArtifacts: ["/app/regex.txt"],
      dependsOn: [1],
      hints: [
        "Use lookahead (?=.*IPv4pattern) to require IPv4 presence",
        "Use boundary assertions (?:^|[^0-9A-Za-z]) to prevent partial matches",
        "Capture only the date part using parentheses",
      ],
      maxTurns: 5,
    },
    {
      id: 3,
      name: "test-and-iterate",
      goal: "Run verify_progress to see test results and fix failures",
      checkpoint: "At least 5/9 test cases passing",
      expectedArtifacts: ["/app/regex.txt"],
      dependsOn: [2],
      hints: [
        "Use verify_progress after each regex update",
        "If false positives: check boundary conditions",
        "If false negatives: check IPv4/date validation is not too strict",
      ],
      maxTurns: 10,
    },
    {
      id: 4,
      name: "final-validation",
      goal: "Ensure all 9 test cases pass",
      checkpoint: "9/9 test cases passing",
      expectedArtifacts: ["/app/regex.txt"],
      dependsOn: [3],
      hints: [
        "Edge cases: Feb has max 29 days, Apr/Jun/Sep/Nov have max 30",
        "Edge IPs: 0.0.0.0 and 255.255.255.255 are valid",
        "Invalid: month 00, day 00, month > 12, day > 31",
      ],
      maxTurns: 7,
    },
  ],
  globalHints: [
    "The regex is applied with re.findall(pattern, text, re.MULTILINE)",
    "Must match ONLY dates in lines that contain a valid IPv4 address",
    "If multiple dates on a line, match only the LAST one",
  ],
  filesToRead: [],
  requiredOutputs: ["/app/regex.txt"],
};

const PATH_TRACING_DECOMPOSITION: TaskDecomposition = {
  taskId: "path-tracing",
  subtaskCount: 4,
  subtasks: [
    {
      id: 1,
      name: "analyze-reference",
      goal: "Read the reference image to understand what needs to be generated",
      checkpoint: "Understand image dimensions and visual content",
      expectedArtifacts: [],
      dependsOn: [],
      hints: [
        "Reference image is at /app/image.ppm",
        "DO NOT read the image data directly (forbidden)",
        "Infer what kind of scene it might be from the task description",
      ],
      maxTurns: 3,
    },
    {
      id: 2,
      name: "write-basic-ppm",
      goal: "Write a C program that outputs a valid 320x200 PPM image",
      checkpoint: "image.c compiles and produces a valid PPM file",
      expectedArtifacts: ["/app/image.c"],
      dependsOn: [1],
      hints: [
        "PPM P6 format: 'P6\\n320 200\\n255\\n' followed by binary RGB data",
        "Compile with: gcc -static -o image image.c -lm",
        "Keep code short (must be < 2KB when gzipped)",
      ],
      maxTurns: 5,
    },
    {
      id: 3,
      name: "implement-rendering",
      goal: "Implement path tracing or ray tracing algorithm to match reference",
      checkpoint: "Generated image has some similarity to reference",
      expectedArtifacts: ["/app/image.c", "/app/reconstructed.ppm"],
      dependsOn: [2],
      hints: [
        "Simple approach: gradient background + basic shapes",
        "For path tracing: implement ray-sphere intersection",
        "Use math.h functions: sqrt, pow, sin, cos",
      ],
      maxTurns: 10,
    },
    {
      id: 4,
      name: "optimize-similarity",
      goal: "Tune parameters to achieve >= 0.99 similarity",
      checkpoint: "Verification passes with >= 0.99 cosine similarity",
      expectedArtifacts: ["/app/image.c", "/app/reconstructed.ppm"],
      dependsOn: [3],
      hints: [
        "Similarity is computed on flattened pixel arrays",
        "Small color differences matter - try adjusting RGB values",
        "Code size matters - compress/optimize if needed",
      ],
      maxTurns: 7,
    },
  ],
  globalHints: [
    "Output must be written to /app/reconstructed.ppm",
    "Cannot read reference image file - must generate from algorithm",
    "Code must be < 2100 bytes when compressed with gzip",
  ],
  filesToRead: [],
  requiredOutputs: ["/app/image.c", "/app/reconstructed.ppm"],
};

const MODEL_EXTRACTION_DECOMPOSITION: TaskDecomposition = {
  taskId: "model-extraction-relu-logits",
  subtaskCount: 4,
  subtasks: [
    {
      id: 1,
      name: "understand-network",
      goal: "Understand the ReLU network architecture and forward() function",
      checkpoint: "Know input dimension (10) and how to query the network",
      expectedArtifacts: [],
      dependsOn: [],
      hints: [
        "Network: output = A2 * ReLU(A1*x + b1) + b2",
        "Input dimension: 10, output dimension: 1",
        "Query with: from forward import forward; y = forward(x)",
      ],
      maxTurns: 3,
    },
    {
      id: 2,
      name: "implement-query-strategy",
      goal: "Write steal.py with a strategy to extract A1 matrix",
      checkpoint: "steal.py runs and makes queries to forward()",
      expectedArtifacts: ["/app/steal.py"],
      dependsOn: [1],
      hints: [
        "Try unit vectors: x = [1,0,0,...], x = [0,1,0,...], etc.",
        "ReLU activation means negative inputs become 0",
        "Use positive and negative probes to detect weight signs",
      ],
      maxTurns: 8,
    },
    {
      id: 3,
      name: "extract-weights",
      goal: "Extract all 30 rows of A1 matrix",
      checkpoint: "stolen_A1.npy exists with shape (30, 10)",
      expectedArtifacts: ["/app/steal.py", "/app/stolen_A1.npy"],
      dependsOn: [2],
      hints: [
        "A1 has 30 rows (hidden layer size)",
        "Each row can be extracted independently",
        "Rows match up to scaling and permutation",
      ],
      maxTurns: 10,
    },
    {
      id: 4,
      name: "verify-extraction",
      goal: "Verify all 30 rows match the original (up to scaling)",
      checkpoint: "All rows matched with ratio difference < 1e-4",
      expectedArtifacts: ["/app/steal.py", "/app/stolen_A1.npy"],
      dependsOn: [3],
      hints: [
        "Verification checks each row against original",
        "Matching is up to permutation and scaling",
        "Tolerance: ratio differences < 1e-4",
      ],
      maxTurns: 5,
    },
  ],
  globalHints: [
    "Network is generated with np.random.seed(5)",
    "Can make unlimited queries to forward()",
    "Output must be saved to /app/stolen_A1.npy",
  ],
  filesToRead: [],
  requiredOutputs: ["/app/steal.py", "/app/stolen_A1.npy"],
};

const VIDEO_PROCESSING_DECOMPOSITION: TaskDecomposition = {
  taskId: "video-processing",
  subtaskCount: 4,
  subtasks: [
    {
      id: 1,
      name: "analyze-video",
      goal: "Read example video and understand jump detection requirements",
      checkpoint: "Understand video format and what constitutes a jump",
      expectedArtifacts: [],
      dependsOn: [],
      hints: [
        "Example video at /app/example_video.mp4",
        "Background and camera are constant",
        "Detect single jump event per video",
      ],
      maxTurns: 3,
    },
    {
      id: 2,
      name: "write-analyzer",
      goal: "Write jump_analyzer.py using OpenCV for motion detection",
      checkpoint: "Script runs without errors on example video",
      expectedArtifacts: ["/app/jump_analyzer.py"],
      dependsOn: [1],
      hints: [
        "Use cv2.VideoCapture to read frames",
        "Detect motion by frame differencing",
        "Only allowed imports: cv2, numpy, toml",
      ],
      maxTurns: 8,
    },
    {
      id: 3,
      name: "detect-takeoff-landing",
      goal: "Accurately detect takeoff and landing frames",
      checkpoint: "Frames within expected ranges for example video",
      expectedArtifacts: ["/app/jump_analyzer.py", "/app/output.toml"],
      dependsOn: [2],
      hints: [
        "Takeoff: first significant upward motion",
        "Landing: motion settles after peak",
        "Example video: takeoff ~50-54, landing ~62-64",
      ],
      maxTurns: 10,
    },
    {
      id: 4,
      name: "output-toml",
      goal: "Write correct TOML output format",
      checkpoint: "output.toml has correct structure and values",
      expectedArtifacts: ["/app/jump_analyzer.py", "/app/output.toml"],
      dependsOn: [3],
      hints: [
        "Format: jump_takeoff_frame_number = N",
        "Format: jump_land_frame_number = M",
        "Use Python toml library to write",
      ],
      maxTurns: 4,
    },
  ],
  globalHints: [
    "Script takes MP4 path as command-line argument",
    "Only imports allowed: cv2, numpy, toml, standard library",
    "Output to /app/output.toml",
  ],
  filesToRead: [],
  requiredOutputs: ["/app/jump_analyzer.py"],
};

const DNA_ASSEMBLY_DECOMPOSITION: TaskDecomposition = {
  taskId: "dna-assembly",
  subtaskCount: 4,
  subtasks: [
    {
      id: 1,
      name: "understand-golden-gate",
      goal: "Understand Golden Gate assembly and BsaI-HF v2 enzyme requirements",
      checkpoint: "Know BsaI cut site structure and assembly requirements",
      expectedArtifacts: [],
      dependsOn: [],
      hints: [
        "BsaI site: [clamp]ggtctc[overhang:4nt][binding]",
        "Need forward and reverse primers for each template",
        "Templates: input, egfp, flag, snap",
      ],
      maxTurns: 3,
    },
    {
      id: 2,
      name: "read-sequences",
      goal: "Read sequences.fasta to understand input/output sequences",
      checkpoint: "Know the sequences for input plasmid and fragments",
      expectedArtifacts: [],
      dependsOn: [1],
      hints: [
        "Input file: /app/sequences.fasta",
        "Contains: input (circular), egfp, flag, snap, output",
        "Need to find junction points in output",
      ],
      maxTurns: 3,
    },
    {
      id: 3,
      name: "design-primers",
      goal: "Design 8 primers with correct BsaI sites and overhangs",
      checkpoint: "primers.fasta has 8 valid primers",
      expectedArtifacts: ["/app/primers.fasta"],
      dependsOn: [2],
      hints: [
        "Format: >TEMPLATENAME_DIR (e.g., >EGFP_fwd)",
        "Annealing region: 15-45 nt",
        "Tm: 58-72 C, pair difference <= 5 C",
      ],
      maxTurns: 10,
    },
    {
      id: 4,
      name: "verify-assembly",
      goal: "Verify primers assemble correctly into output plasmid",
      checkpoint: "All primers valid, Tm correct, assembly matches output",
      expectedArtifacts: ["/app/primers.fasta"],
      dependsOn: [3],
      hints: [
        "Use oligotm for Tm calculation",
        "Check overhang uniqueness",
        "Verify circular assembly closure",
      ],
      maxTurns: 5,
    },
  ],
  globalHints: [
    "Output: /app/primers.fasta with exactly 16 lines (8 primers)",
    "Sequences must be lowercase, headers capitalized",
    "Tm calculated with: oligotm -tp 1 -sc 1 -mv 50 -dv 2 -n 0.8 -d 500",
  ],
  filesToRead: ["/app/sequences.fasta"],
  requiredOutputs: ["/app/primers.fasta"],
};

// ============================================================================
// Decomposition Registry
// ============================================================================

const DECOMPOSITIONS: Record<string, TaskDecomposition> = {
  "regex-log": REGEX_LOG_DECOMPOSITION,
  "path-tracing": PATH_TRACING_DECOMPOSITION,
  "model-extraction-relu-logits": MODEL_EXTRACTION_DECOMPOSITION,
  "video-processing": VIDEO_PROCESSING_DECOMPOSITION,
  "dna-assembly": DNA_ASSEMBLY_DECOMPOSITION,
};

// ============================================================================
// Main Decomposer
// ============================================================================

/**
 * Decompose a Terminal-Bench task into subtasks.
 *
 * @param task Terminal-Bench task
 * @returns Task decomposition with subtasks, or null if no decomposition exists
 */
export function decomposeTask(task: TerminalBenchTask): TaskDecomposition | null {
  return DECOMPOSITIONS[task.id] ?? null;
}

/**
 * Get the current subtask based on execution state.
 *
 * @param decomposition Task decomposition
 * @param completedSubtasks IDs of completed subtasks
 * @returns Current subtask to work on, or null if all complete
 */
export function getCurrentSubtask(
  decomposition: TaskDecomposition,
  completedSubtasks: number[]
): Subtask | null {
  for (const subtask of decomposition.subtasks) {
    // Check if already completed
    if (completedSubtasks.includes(subtask.id)) {
      continue;
    }

    // Check if dependencies are met
    const depsComplete = subtask.dependsOn.every((dep) => completedSubtasks.includes(dep));
    if (depsComplete) {
      return subtask;
    }
  }

  return null;
}

/**
 * Build a prompt for the current subtask.
 */
export function buildSubtaskPrompt(
  decomposition: TaskDecomposition,
  subtask: Subtask,
  previousFeedback?: string
): string {
  const lines: string[] = [];

  lines.push(`## Current Subtask: ${subtask.name} (${subtask.id}/${decomposition.subtaskCount})`);
  lines.push("");
  lines.push(`**Goal:** ${subtask.goal}`);
  lines.push("");
  lines.push(`**Checkpoint:** ${subtask.checkpoint}`);
  lines.push("");

  if (subtask.hints.length > 0) {
    lines.push("**Hints:**");
    for (const hint of subtask.hints) {
      lines.push(`- ${hint}`);
    }
    lines.push("");
  }

  if (subtask.expectedArtifacts.length > 0) {
    lines.push(`**Expected outputs:** ${subtask.expectedArtifacts.join(", ")}`);
    lines.push("");
  }

  if (previousFeedback) {
    lines.push("**Previous attempt feedback:**");
    lines.push(previousFeedback);
    lines.push("");
  }

  return lines.join("\n");
}

/**
 * Check if a subtask is complete based on evaluation results.
 */
export function isSubtaskComplete(
  subtask: Subtask,
  progress: number,
  artifacts: string[]
): boolean {
  // Check if all expected artifacts exist
  const hasAllArtifacts = subtask.expectedArtifacts.every((a) =>
    artifacts.some((artifact) => artifact.endsWith(a) || artifact === a)
  );

  if (!hasAllArtifacts) {
    return false;
  }

  // For final subtasks, require full progress
  if (subtask.checkpoint.includes("pass") && progress < 1) {
    return false;
  }

  // For intermediate subtasks, check progress threshold
  // Subtask 3 (test-and-iterate) needs at least 50% progress
  if (subtask.id === 3 && progress < 0.5) {
    return false;
  }

  return true;
}

/**
 * Create a fallback decomposition for unknown tasks.
 */
export function createFallbackDecomposition(task: TerminalBenchTask): TaskDecomposition {
  return {
    taskId: task.id,
    subtaskCount: 3,
    subtasks: [
      {
        id: 1,
        name: "understand",
        goal: "Read and understand the task requirements",
        checkpoint: "Task requirements are clear",
        expectedArtifacts: [],
        dependsOn: [],
        hints: ["Read the task description carefully", "Identify input and output files"],
        maxTurns: 3,
      },
      {
        id: 2,
        name: "implement",
        goal: "Implement the solution",
        checkpoint: "Solution file exists",
        expectedArtifacts: [],
        dependsOn: [1],
        hints: ["Write the solution code", "Use verify_progress to check progress"],
        maxTurns: 15,
      },
      {
        id: 3,
        name: "verify",
        goal: "Verify the solution passes all tests",
        checkpoint: "All tests pass",
        expectedArtifacts: [],
        dependsOn: [2],
        hints: ["Run verification", "Fix any remaining issues"],
        maxTurns: 10,
      },
    ],
    globalHints: [task.description.slice(0, 200)],
    filesToRead: [],
    requiredOutputs: [],
  };
}
