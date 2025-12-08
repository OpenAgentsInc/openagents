/**
 * TB2 Structural Skills
 *
 * Domain knowledge patterns for Terminal-Bench 2 tasks.
 * These are NOT solutions - they're structural patterns that help FM
 * understand the domain and iterate effectively.
 *
 * Based on the plan: "NOT the actual solutions, just structural patterns"
 */

import { createSkill, type Skill } from "../schema.js";

// ============================================================================
// Regex Skills (for regex-log task)
// ============================================================================

export const REGEX_BOUNDARY_SKILL: Skill = createSkill({
  name: "regex-boundary-assertions",
  description: "Use word boundary assertions to match patterns not surrounded by alphanumeric characters",
  category: "file_operations",
  code: `// Boundary assertions prevent false matches
// (?:^|[^0-9A-Za-z]) - matches start of string OR non-alphanumeric
// (?=$|[^0-9A-Za-z]) - followed by end of string OR non-alphanumeric

// Example: Match numbers not embedded in words
const pattern = /(?:^|[^0-9A-Za-z])(\\d+)(?=$|[^0-9A-Za-z])/g;

// For IPv4, ensure octets are 0-255:
// (?:25[0-5]|2[0-4]\\d|1?\\d?\\d) matches 0-255`,
  tags: ["regex", "patterns", "validation"],
  languages: ["regex"],
});

export const REGEX_LOOKAHEAD_SKILL: Skill = createSkill({
  name: "regex-lookahead-constraints",
  description: "Use lookahead assertions to require patterns without consuming them",
  category: "file_operations",
  code: `// Positive lookahead (?=...) requires pattern ahead without consuming
// Useful for: "match X only if Y exists on the line"

// Example: Match date only if IPv4 exists on same line
const pattern = /(?=.*IPv4_PATTERN).*DATE_PATTERN/;

// Negative lookahead (?!...) requires pattern NOT ahead
// Example: Match word not followed by colon
const pattern = /\\w+(?!:)/;

// Lookbehind (?<=...) and (?<!...) work similarly backwards`,
  tags: ["regex", "patterns", "lookahead"],
  languages: ["regex"],
});

export const REGEX_DATE_VALIDATION_SKILL: Skill = createSkill({
  name: "regex-date-validation",
  description: "Validate dates in YYYY-MM-DD format with correct month/day ranges",
  category: "file_operations",
  code: `// Valid dates need:
// - Year: \\d{4} (any 4 digits)
// - Month: 0[1-9]|1[0-2] (01-12)
// - Day: depends on month, but basic: 0[1-9]|[12]\\d|3[01]

// Simple pattern (may allow invalid like Feb 31):
const basicDate = /\\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\\d|3[01])/;

// For strict validation, consider month-specific patterns
// or validate programmatically after matching`,
  tags: ["regex", "patterns", "validation", "dates"],
  languages: ["regex"],
});

// ============================================================================
// C Graphics Skills (for path-tracing task)
// ============================================================================

export const PPM_FORMAT_SKILL: Skill = createSkill({
  name: "ppm-image-format",
  description: "Write images in PPM format (P6 binary or P3 ASCII)",
  category: "file_operations",
  code: `// PPM P6 (binary) format:
// Header: "P6\\n<width> <height>\\n<maxval>\\n"
// Body: width*height RGB triplets (3 bytes each)

printf("P6\\n%d %d\\n255\\n", width, height);
for (int y = 0; y < height; y++) {
    for (int x = 0; x < width; x++) {
        unsigned char rgb[3] = {r, g, b};
        fwrite(rgb, 1, 3, stdout);
    }
}

// PPM P3 (ASCII) format - easier to debug:
printf("P3\\n%d %d\\n255\\n", width, height);
for (int y = 0; y < height; y++) {
    for (int x = 0; x < width; x++) {
        printf("%d %d %d ", r, g, b);
    }
    printf("\\n");
}`,
  tags: ["c", "graphics", "ppm", "image"],
  languages: ["c"],
});

export const RAY_SPHERE_INTERSECTION_SKILL: Skill = createSkill({
  name: "ray-sphere-intersection",
  description: "Calculate ray-sphere intersection for path tracing",
  category: "file_operations",
  code: `// Ray: P(t) = origin + t * direction
// Sphere: |P - center|^2 = radius^2
// Substitute and solve quadratic

typedef struct { float x, y, z; } vec3;

float dot(vec3 a, vec3 b) { return a.x*b.x + a.y*b.y + a.z*b.z; }
vec3 sub(vec3 a, vec3 b) { return (vec3){a.x-b.x, a.y-b.y, a.z-b.z}; }

float intersect_sphere(vec3 origin, vec3 dir, vec3 center, float radius) {
    vec3 oc = sub(origin, center);
    float a = dot(dir, dir);
    float b = 2.0f * dot(oc, dir);
    float c = dot(oc, oc) - radius * radius;
    float disc = b*b - 4*a*c;
    if (disc < 0) return -1;
    return (-b - sqrtf(disc)) / (2*a);
}`,
  tags: ["c", "graphics", "raytracing", "math"],
  languages: ["c"],
});

// ============================================================================
// Python ML Skills (for model-extraction task)
// ============================================================================

export const NUMPY_ARRAY_MANIPULATION_SKILL: Skill = createSkill({
  name: "numpy-array-operations",
  description: "Common NumPy patterns for matrix manipulation",
  category: "file_operations",
  code: `import numpy as np

# Unit vectors for probing neural networks
eye = np.eye(n)  # Identity matrix - each row is a unit vector

# Query with batches
inputs = np.vstack([eye, -eye])  # Both positive and negative directions

# Extract weights from linear layer output
# If f(x) = Ax + b, then:
# f(e_i) - f(0) = A[:,i]  (column i of weight matrix)

# ReLU handling: max(0, x)
# To get weights through ReLU, need multiple probes
# Try both positive and negative inputs`,
  tags: ["python", "numpy", "ml", "arrays"],
  languages: ["python"],
});

export const MODEL_EXTRACTION_PATTERNS_SKILL: Skill = createSkill({
  name: "model-extraction-patterns",
  description: "Patterns for extracting weights from black-box neural networks",
  category: "file_operations",
  code: `# For linear layer y = Wx + b:
# 1. Get bias: b = model(zeros)
# 2. Get columns: W[:,i] = model(e_i) - b

# For ReLU activation:
# - Pre-activation values can be negative
# - Use both e_i and -e_i to probe
# - Compare results to identify which neurons are active

# Save extracted weights
np.save('stolen_A1.npy', extracted_weights)

# Verify extraction
tolerance = 1e-6
match = np.allclose(extracted, actual, atol=tolerance)`,
  tags: ["python", "ml", "extraction", "neural-networks"],
  languages: ["python"],
});

// ============================================================================
// Python Video Skills (for video-processing task)
// ============================================================================

export const VIDEO_FRAME_ANALYSIS_SKILL: Skill = createSkill({
  name: "video-frame-analysis",
  description: "Analyze video frames for motion detection using OpenCV",
  category: "file_operations",
  code: `import cv2
import numpy as np

cap = cv2.VideoCapture('video.mp4')
fps = cap.get(cv2.CAP_PROP_FPS)

prev_frame = None
while True:
    ret, frame = cap.read()
    if not ret:
        break

    # Convert to grayscale for motion detection
    gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
    gray = cv2.GaussianBlur(gray, (21, 21), 0)

    if prev_frame is not None:
        # Frame difference
        diff = cv2.absdiff(prev_frame, gray)
        thresh = cv2.threshold(diff, 25, 255, cv2.THRESH_BINARY)[1]
        motion_score = np.sum(thresh) / thresh.size

        # Detect significant motion (jump detection)
        if motion_score > threshold:
            frame_number = int(cap.get(cv2.CAP_PROP_POS_FRAMES))
            timestamp = frame_number / fps

    prev_frame = gray`,
  tags: ["python", "opencv", "video", "motion"],
  languages: ["python"],
});

export const TOML_OUTPUT_SKILL: Skill = createSkill({
  name: "toml-output-format",
  description: "Write results in TOML format",
  category: "file_operations",
  code: `# TOML format for structured output
# Use for configuration files and data exchange

import toml

results = {
    'metadata': {
        'video_file': 'input.mp4',
        'fps': 30.0
    },
    'jumps': [
        {'start_frame': 120, 'end_frame': 150, 'timestamp': 4.0},
        {'start_frame': 450, 'end_frame': 480, 'timestamp': 15.0},
    ]
}

with open('output.toml', 'w') as f:
    toml.dump(results, f)

# Or simple manual output:
print(f'[[jumps]]')
print(f'start_frame = {start}')
print(f'end_frame = {end}')`,
  tags: ["python", "toml", "output", "formatting"],
  languages: ["python"],
});

// ============================================================================
// DNA/Bio Skills (for dna-assembly task)
// ============================================================================

export const BIOPYTHON_PATTERNS_SKILL: Skill = createSkill({
  name: "biopython-patterns",
  description: "BioPython patterns for DNA sequence manipulation",
  category: "file_operations",
  code: `from Bio.Seq import Seq
from Bio.SeqRecord import SeqRecord
from Bio import SeqIO

# Create sequence
seq = Seq("ATGCGATCGATCGATCG")

# Reverse complement
rev_comp = seq.reverse_complement()

# Find restriction sites
# BsaI cuts: GGTCTC(N1/N5) - leaves 4bp overhang
bsai_site = "GGTCTC"
position = str(seq).find(bsai_site)

# Create FASTA record
record = SeqRecord(
    seq,
    id="primer_1",
    description="Forward primer for fragment 1"
)

# Write FASTA
with open('primers.fasta', 'w') as f:
    SeqIO.write([record1, record2, ...], f, 'fasta')`,
  tags: ["python", "biopython", "dna", "sequences"],
  languages: ["python"],
});

export const PRIMER_DESIGN_SKILL: Skill = createSkill({
  name: "primer-design-basics",
  description: "Basic rules for PCR primer design",
  category: "file_operations",
  code: `# Golden Gate Assembly primer design:

# 1. Add BsaI recognition site: GGTCTC
# 2. Add 4bp overhang after cut site
# 3. Add annealing region (15-25bp from template)

# Structure:
# [5' tail] - [BsaI site: GGTCTC] - [N] - [4bp overhang] - [annealing region]

# Melting temperature (Tm) estimation:
# Simple: Tm = 4*(G+C) + 2*(A+T)  (for primers <14bp)
# Better: Tm = 64.9 + 41*(G+C-16.4)/(A+T+G+C)

# Overhangs must be:
# - 4 nucleotides long
# - Unique (no two fragments with same overhang)
# - Not self-complementary`,
  tags: ["biology", "primers", "pcr", "golden-gate"],
  languages: ["python"],
});

export const FASTA_FORMAT_SKILL: Skill = createSkill({
  name: "fasta-format",
  description: "FASTA file format for biological sequences",
  category: "file_operations",
  code: `# FASTA format:
# >header_line (description, ID, etc.)
# SEQUENCE_DATA
# (sequence can span multiple lines, typically 60-80 chars/line)

# Example primers.fasta:
>primer_1_fwd Description here
ATGCGGTCTCAAACTATGCGATCGATCG
>primer_1_rev
ATGCGGTCTCATTTGTAGCTAGCTAGCT
>primer_2_fwd
ATGCGGTCTCAAAATCGATCGATCGATC
>primer_2_rev
ATGCGGTCTCATTTTCGATCGATCGATC

# For Golden Gate with 4 fragments:
# Need 8 primers total (forward and reverse for each fragment)
# Each pair creates one fragment with specific overhangs`,
  tags: ["biology", "fasta", "sequences", "formatting"],
  languages: ["plaintext"],
});

// ============================================================================
// Skill Collection Export
// ============================================================================

/**
 * All TB2 structural skills.
 * These are patterns, NOT solutions.
 */
export const TB2_SKILLS: Skill[] = [
  // Regex skills
  REGEX_BOUNDARY_SKILL,
  REGEX_LOOKAHEAD_SKILL,
  REGEX_DATE_VALIDATION_SKILL,
  // C graphics skills
  PPM_FORMAT_SKILL,
  RAY_SPHERE_INTERSECTION_SKILL,
  // Python ML skills
  NUMPY_ARRAY_MANIPULATION_SKILL,
  MODEL_EXTRACTION_PATTERNS_SKILL,
  // Python video skills
  VIDEO_FRAME_ANALYSIS_SKILL,
  TOML_OUTPUT_SKILL,
  // DNA/Bio skills
  BIOPYTHON_PATTERNS_SKILL,
  PRIMER_DESIGN_SKILL,
  FASTA_FORMAT_SKILL,
];

/**
 * Get skills relevant to a specific task.
 */
export function getSkillsForTask(taskId: string): Skill[] {
  const taskSkillMap: Record<string, Skill[]> = {
    "regex-log": [REGEX_BOUNDARY_SKILL, REGEX_LOOKAHEAD_SKILL, REGEX_DATE_VALIDATION_SKILL],
    "path-tracing": [PPM_FORMAT_SKILL, RAY_SPHERE_INTERSECTION_SKILL],
    "model-extraction-relu-logits": [NUMPY_ARRAY_MANIPULATION_SKILL, MODEL_EXTRACTION_PATTERNS_SKILL],
    "video-processing": [VIDEO_FRAME_ANALYSIS_SKILL, TOML_OUTPUT_SKILL],
    "dna-assembly": [BIOPYTHON_PATTERNS_SKILL, PRIMER_DESIGN_SKILL, FASTA_FORMAT_SKILL],
  };

  return taskSkillMap[taskId] ?? [];
}

/**
 * Format TB2 skills as hints for FM prompt.
 * Shorter format than full skill display.
 */
export function formatTB2SkillsAsHints(skills: Skill[]): string {
  if (skills.length === 0) return "";

  const hints = skills.map((s) => {
    // Just name and a one-liner
    const firstLine = s.description.split("\n")[0];
    return `- ${s.name}: ${firstLine}`;
  });

  return `Domain Knowledge:\n${hints.join("\n")}`;
}
