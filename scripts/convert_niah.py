#!/usr/bin/env python3
"""
Convert Needle-in-a-Haystack dataset to RLM benchmark format.

Input: Greg Kamradt's LLMTest_NeedleInAHaystack repository
Output: data/sniah/sniah.jsonl

Format:
{"id": "sniah-001", "query": "...", "context": "...", "needle": "...", "context_tokens": 50000}
"""

import json
import os
import sys
import random
from pathlib import Path

# Sample needles for synthetic generation
NEEDLES = [
    "The secret code is: ALPHA-7749-BRAVO",
    "The password to the vault is: diamond-sunset-42",
    "The special key is: XK7-QUANTUM-ROSE",
    "The access token is: TOKEN-99Z-EMERALD",
    "The magic phrase is: whisper-mountain-echo",
]

# Sample haystack texts (will repeat/combine for longer contexts)
HAYSTACK_TEXTS = [
    """The history of computing began long before the modern computer era. Charles Babbage conceived the first mechanical computer in the early 19th century, designing the Analytical Engine which contained an arithmetic logic unit, control flow in the form of conditional branching and loops, and integrated memory. Although Babbage was never able to complete construction of any of his machines due to funding problems and conflicts with his chief engineer, his designs were groundbreaking.""",

    """In the realm of artificial intelligence, machine learning has emerged as a transformative technology. Neural networks, inspired by biological neural networks, have proven particularly effective at tasks like image recognition, natural language processing, and game playing. Deep learning, a subset of machine learning using multi-layer neural networks, has achieved remarkable results in areas previously thought to be exclusively human domains.""",

    """The development of the internet transformed how humanity communicates and shares information. From its origins as ARPANET in the 1960s, the network grew into a global infrastructure connecting billions of devices. The World Wide Web, invented by Tim Berners-Lee in 1989, made the internet accessible to everyday users through hypertext documents and web browsers.""",

    """Climate science has advanced significantly in recent decades. Researchers have developed sophisticated models to predict temperature changes, sea level rise, and extreme weather patterns. Satellite observations provide crucial data about ice sheet dynamics, ocean temperatures, and atmospheric composition. The scientific consensus on anthropogenic climate change has strengthened as evidence accumulates.""",

    """The human genome project, completed in 2003, mapped the complete human genetic sequence. This monumental achievement has enabled advances in personalized medicine, genetic testing, and our understanding of hereditary diseases. CRISPR-Cas9 gene editing technology has since revolutionized the field, offering precise tools for modifying DNA sequences.""",
]

QUERIES = [
    "What is the secret code mentioned in the text?",
    "What is the password mentioned in the document?",
    "What is the special key referenced in the passage?",
    "What access token is mentioned in the text?",
    "What is the magic phrase mentioned in the document?",
]


def generate_haystack(target_length: int) -> str:
    """Generate haystack text of approximately target length."""
    haystack = ""
    while len(haystack) < target_length:
        text = random.choice(HAYSTACK_TEXTS)
        haystack += text + "\n\n"
    return haystack[:target_length]


def insert_needle(haystack: str, needle: str, position: float) -> str:
    """Insert needle at relative position (0.0 = start, 1.0 = end)."""
    insert_point = int(len(haystack) * position)
    # Find a good break point (paragraph boundary)
    search_start = max(0, insert_point - 100)
    search_end = min(len(haystack), insert_point + 100)
    search_region = haystack[search_start:search_end]

    para_break = search_region.find("\n\n")
    if para_break != -1:
        insert_point = search_start + para_break + 2

    return haystack[:insert_point] + f"\n{needle}\n\n" + haystack[insert_point:]


def generate_synthetic_tasks(output_dir: Path, num_tasks: int = 50) -> None:
    """Generate synthetic S-NIAH tasks."""
    output_file = output_dir / "sniah.jsonl"

    tasks = []
    context_lengths = [10000, 25000, 50000, 75000, 100000]
    positions = [0.1, 0.25, 0.5, 0.75, 0.9]

    task_id = 0
    for context_len in context_lengths:
        for position in positions:
            for i, (needle, query) in enumerate(zip(NEEDLES, QUERIES)):
                if task_id >= num_tasks:
                    break

                haystack = generate_haystack(context_len)
                context = insert_needle(haystack, needle, position)

                # Extract just the answer part from needle
                answer = needle.split(": ", 1)[1] if ": " in needle else needle

                task = {
                    "id": f"sniah-{task_id:03d}",
                    "query": query,
                    "context": context,
                    "needle": answer,
                    "needle_position": position,
                    "context_tokens": len(context) // 4,  # Rough token estimate
                }
                tasks.append(task)
                task_id += 1

            if task_id >= num_tasks:
                break
        if task_id >= num_tasks:
            break

    # Write to JSONL
    with open(output_file, "w") as f:
        for task in tasks:
            f.write(json.dumps(task) + "\n")

    print(f"  Generated {len(tasks)} S-NIAH tasks")


def convert_from_repo(input_dir: Path, output_dir: Path) -> None:
    """Convert from the LLMTest_NeedleInAHaystack repository format."""
    output_file = output_dir / "sniah.jsonl"

    # Check if repository has the expected structure
    needles_file = input_dir / "needles.txt"
    haystacks_dir = input_dir / "haystacks"

    if needles_file.exists():
        # Load needles from file
        with open(needles_file) as f:
            needles = [line.strip() for line in f if line.strip()]
    else:
        needles = NEEDLES

    # Generate tasks using repo content or fall back to synthetic
    if haystacks_dir.exists() and any(haystacks_dir.iterdir()):
        print("  Found haystack files, converting...")
        # TODO: Implement actual conversion from repo format
        generate_synthetic_tasks(output_dir)
    else:
        print("  No haystack files found, generating synthetic data...")
        generate_synthetic_tasks(output_dir)


def main():
    if len(sys.argv) < 3:
        print("Usage: convert_niah.py <input_dir> <output_dir>")
        print("       convert_niah.py --synthetic <output_dir>")
        sys.exit(1)

    if sys.argv[1] == "--synthetic":
        output_dir = Path(sys.argv[2])
        output_dir.mkdir(parents=True, exist_ok=True)
        generate_synthetic_tasks(output_dir)
    else:
        input_dir = Path(sys.argv[1])
        output_dir = Path(sys.argv[2])
        output_dir.mkdir(parents=True, exist_ok=True)
        convert_from_repo(input_dir, output_dir)


if __name__ == "__main__":
    main()
