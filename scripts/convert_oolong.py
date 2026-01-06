#!/usr/bin/env python3
"""
Convert OOLONG dataset to RLM benchmark format.

Outputs:
- data/oolong/trec_coarse.jsonl - Numeric counting tasks
- data/oolong/pairs.jsonl - Pairwise aggregation tasks

TrecCoarse format:
{"id": "oolong-trec-001", "query": "How many documents mention X?", "context": "...", "answer": 42}

Pairs format:
{"id": "oolong-pairs-001", "query": "Which topics appear in both?", "context": "...", "answer": ["topic1", "topic2"]}
"""

import json
import random
import sys
from pathlib import Path

# Topics for synthetic generation
TOPICS = [
    "climate change", "artificial intelligence", "renewable energy",
    "cybersecurity", "blockchain", "quantum computing", "biotechnology",
    "space exploration", "autonomous vehicles", "genetic engineering",
    "machine learning", "data privacy", "cloud computing", "robotics",
    "nanotechnology", "virtual reality", "5G networks", "sustainability",
]

# Document templates
DOC_TEMPLATES = [
    "This document discusses {topic}. Recent advances in {topic} have led to significant developments. Researchers continue to explore new applications of {topic} in various fields.",
    "The field of {topic} has seen remarkable growth. Industry leaders are investing heavily in {topic} research. The future of {topic} looks promising with new breakthroughs on the horizon.",
    "{topic} is transforming how we approach complex problems. The integration of {topic} into everyday life continues to accelerate. Experts predict {topic} will become even more important.",
    "Studies on {topic} reveal interesting patterns. The impact of {topic} on society is becoming more apparent. Policy makers are taking note of developments in {topic}.",
]


def generate_document(topics: list[str]) -> str:
    """Generate a synthetic document mentioning the given topics."""
    paragraphs = []
    for topic in topics:
        template = random.choice(DOC_TEMPLATES)
        paragraphs.append(template.format(topic=topic))
    return "\n\n".join(paragraphs)


def generate_trec_coarse_tasks(output_file: Path, num_tasks: int = 30) -> None:
    """Generate OOLONG TrecCoarse (counting) tasks."""
    tasks = []

    for i in range(num_tasks):
        # Pick a target topic to count
        target_topic = random.choice(TOPICS)

        # Generate 5-15 documents
        num_docs = random.randint(5, 15)
        docs = []
        count = 0

        for doc_idx in range(num_docs):
            # Randomly include target topic
            include_target = random.random() < 0.4
            num_topics = random.randint(1, 3)
            doc_topics = random.sample([t for t in TOPICS if t != target_topic], num_topics)

            if include_target:
                doc_topics.append(target_topic)
                count += 1

            doc = generate_document(doc_topics)
            docs.append(f"[Document {doc_idx + 1}]\n{doc}")

        context = "\n\n---\n\n".join(docs)

        task = {
            "id": f"oolong-trec-{i:03d}",
            "query": f"How many documents mention '{target_topic}'?",
            "context": context,
            "answer": count,
            "tolerance": 0,  # Exact match required
        }
        tasks.append(task)

    with open(output_file, "w") as f:
        for task in tasks:
            f.write(json.dumps(task) + "\n")

    print(f"  Generated {len(tasks)} OOLONG TrecCoarse tasks")


def generate_pairs_tasks(output_file: Path, num_tasks: int = 30) -> None:
    """Generate OOLONG Pairs (aggregation) tasks."""
    tasks = []

    for i in range(num_tasks):
        # Select topics for two documents
        all_topics = random.sample(TOPICS, 8)
        doc_a_topics = set(all_topics[:5])
        doc_b_topics = set(all_topics[3:8])  # Overlap of 2 topics

        shared_topics = sorted(doc_a_topics & doc_b_topics)

        doc_a = generate_document(list(doc_a_topics))
        doc_b = generate_document(list(doc_b_topics))

        context = f"[Document A]\n{doc_a}\n\n---\n\n[Document B]\n{doc_b}"

        task = {
            "id": f"oolong-pairs-{i:03d}",
            "query": "Which topics are discussed in both Document A and Document B?",
            "context": context,
            "answer": shared_topics,
        }
        tasks.append(task)

    with open(output_file, "w") as f:
        for task in tasks:
            f.write(json.dumps(task) + "\n")

    print(f"  Generated {len(tasks)} OOLONG Pairs tasks")


def convert_from_repo(input_dir: Path, output_dir: Path) -> None:
    """Convert from the official OOLONG repository."""
    trec_file = output_dir / "trec_coarse.jsonl"
    pairs_file = output_dir / "pairs.jsonl"

    # Check for expected files in the repository
    trec_src = input_dir / "data" / "trec_coarse.jsonl"
    pairs_src = input_dir / "data" / "pairs.jsonl"

    if trec_src.exists():
        print("  Converting TrecCoarse from repository...")
        # TODO: Implement actual conversion
        generate_trec_coarse_tasks(trec_file)
    else:
        print("  TrecCoarse not found, generating synthetic...")
        generate_trec_coarse_tasks(trec_file)

    if pairs_src.exists():
        print("  Converting Pairs from repository...")
        # TODO: Implement actual conversion
        generate_pairs_tasks(pairs_file)
    else:
        print("  Pairs not found, generating synthetic...")
        generate_pairs_tasks(pairs_file)


def main():
    if len(sys.argv) < 2:
        print("Usage: convert_oolong.py <input_dir> <output_dir>")
        print("       convert_oolong.py --synthetic <output_dir>")
        sys.exit(1)

    if sys.argv[1] == "--synthetic":
        output_dir = Path(sys.argv[2])
        output_dir.mkdir(parents=True, exist_ok=True)
        generate_trec_coarse_tasks(output_dir / "trec_coarse.jsonl")
        generate_pairs_tasks(output_dir / "pairs.jsonl")
    else:
        input_dir = Path(sys.argv[1])
        output_dir = Path(sys.argv[2])
        output_dir.mkdir(parents=True, exist_ok=True)
        convert_from_repo(input_dir, output_dir)


if __name__ == "__main__":
    main()
