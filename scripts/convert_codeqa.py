#!/usr/bin/env python3
"""
Convert LongBench v2 CodeQA dataset to RLM benchmark format.

Input: THUDM/LongBench dataset from HuggingFace
Output: data/codeqa/codeqa.jsonl

Format:
{"id": "codeqa-001", "question": "...", "code": "...", "choices": ["A. ...", "B. ...", "C. ...", "D. ..."], "answer": "B", "language": "python"}
"""

import json
import random
import sys
from pathlib import Path

# Sample code snippets for synthetic generation
CODE_SNIPPETS = {
    "python": [
        {
            "code": '''def calculate_factorial(n):
    """Calculate factorial of n."""
    if n <= 1:
        return 1
    result = 1
    for i in range(2, n + 1):
        result *= i
    return result

def main():
    numbers = [5, 7, 10]
    for num in numbers:
        print(f"factorial({num}) = {calculate_factorial(num)}")

if __name__ == "__main__":
    main()''',
            "question": "What does the calculate_factorial function return when called with n=0?",
            "choices": ["A. 0", "B. 1", "C. None", "D. Raises an error"],
            "answer": "B",
        },
        {
            "code": '''class Stack:
    def __init__(self):
        self.items = []

    def push(self, item):
        self.items.append(item)

    def pop(self):
        if not self.items:
            return None
        return self.items.pop()

    def peek(self):
        if not self.items:
            return None
        return self.items[-1]

    def is_empty(self):
        return len(self.items) == 0

stack = Stack()
stack.push(1)
stack.push(2)
stack.push(3)
result = stack.pop()''',
            "question": "What is the value of 'result' after executing this code?",
            "choices": ["A. 1", "B. 2", "C. 3", "D. None"],
            "answer": "C",
        },
        {
            "code": '''def binary_search(arr, target):
    left, right = 0, len(arr) - 1
    while left <= right:
        mid = (left + right) // 2
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid + 1
        else:
            right = mid - 1
    return -1

sorted_list = [1, 3, 5, 7, 9, 11, 13, 15]
index = binary_search(sorted_list, 7)''',
            "question": "What is the value of 'index' after executing binary_search?",
            "choices": ["A. 2", "B. 3", "C. 4", "D. -1"],
            "answer": "B",
        },
    ],
    "javascript": [
        {
            "code": '''function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}

let counter = 0;
const increment = debounce(() => counter++, 100);
increment();
increment();
increment();
// After 100ms passes...''',
            "question": "After the debounce timer expires, what will be the value of counter?",
            "choices": ["A. 0", "B. 1", "C. 3", "D. undefined"],
            "answer": "B",
        },
        {
            "code": '''const arr = [1, 2, 3, 4, 5];
const result = arr.reduce((acc, val) => {
    if (val % 2 === 0) {
        return acc + val * 2;
    }
    return acc;
}, 0);''',
            "question": "What is the value of 'result'?",
            "choices": ["A. 6", "B. 12", "C. 15", "D. 30"],
            "answer": "B",
        },
    ],
    "rust": [
        {
            "code": '''fn main() {
    let mut vec = vec![1, 2, 3];
    let first = &vec[0];
    vec.push(4);
    println!("{}", first);
}''',
            "question": "What happens when this Rust code is compiled?",
            "choices": [
                "A. Prints 1",
                "B. Prints 4",
                "C. Compilation error: cannot borrow as mutable",
                "D. Runtime panic"
            ],
            "answer": "C",
        },
        {
            "code": '''fn process(x: Option<i32>) -> i32 {
    match x {
        Some(n) if n > 0 => n * 2,
        Some(n) => n,
        None => 0,
    }
}

fn main() {
    let a = process(Some(5));
    let b = process(Some(-3));
    let c = process(None);
    let result = a + b + c;
}''',
            "question": "What is the value of 'result'?",
            "choices": ["A. 2", "B. 5", "C. 7", "D. 10"],
            "answer": "C",
        },
    ],
}

# Additional longer code samples for more realistic benchmarks
LONG_CODE_TEMPLATES = {
    "python": '''"""
Module: {module_name}
Purpose: {purpose}
"""

import logging
from typing import List, Dict, Optional, Any
from dataclasses import dataclass
from collections import defaultdict

logger = logging.getLogger(__name__)

@dataclass
class {class_name}:
    """Represents a {entity_desc}."""
    id: str
    name: str
    value: float
    metadata: Dict[str, Any]

    def validate(self) -> bool:
        """Check if the entity is valid."""
        return bool(self.id and self.name and self.value >= 0)

class {processor_name}:
    """Processes {entity_desc} instances."""

    def __init__(self, config: Optional[Dict[str, Any]] = None):
        self.config = config or {{}}
        self._cache: Dict[str, {class_name}] = {{}}
        self._stats: Dict[str, int] = defaultdict(int)

    def add(self, item: {class_name}) -> bool:
        """Add an item to the processor."""
        if not item.validate():
            logger.warning(f"Invalid item: {{item.id}}")
            return False

        self._cache[item.id] = item
        self._stats["added"] += 1
        return True

    def get(self, item_id: str) -> Optional[{class_name}]:
        """Retrieve an item by ID."""
        self._stats["lookups"] += 1
        return self._cache.get(item_id)

    def process_batch(self, items: List[{class_name}]) -> Dict[str, int]:
        """Process a batch of items."""
        results = {{"success": 0, "failed": 0}}

        for item in items:
            if self.add(item):
                results["success"] += 1
            else:
                results["failed"] += 1

        return results

    def get_stats(self) -> Dict[str, int]:
        """Return processing statistics."""
        return dict(self._stats)

    def clear(self) -> None:
        """Clear all cached items."""
        self._cache.clear()
        self._stats["cleared"] += 1


def main():
    processor = {processor_name}()

    items = [
        {class_name}("1", "first", 10.0, {{"type": "A"}}),
        {class_name}("2", "second", 20.0, {{"type": "B"}}),
        {class_name}("3", "", -5.0, {{}}),  # Invalid
    ]

    results = processor.process_batch(items)
    print(f"Processed: {{results}}")
    print(f"Stats: {{processor.get_stats()}}")


if __name__ == "__main__":
    main()
''',
}


def generate_synthetic_tasks(output_file: Path, num_tasks: int = 50) -> None:
    """Generate synthetic CodeQA tasks."""
    tasks = []
    task_id = 0

    # Use predefined snippets
    for lang, snippets in CODE_SNIPPETS.items():
        for snippet in snippets:
            if task_id >= num_tasks:
                break

            task = {
                "id": f"codeqa-{task_id:03d}",
                "question": snippet["question"],
                "code": snippet["code"],
                "choices": snippet["choices"],
                "answer": snippet["answer"],
                "language": lang,
            }
            tasks.append(task)
            task_id += 1

    # Generate more tasks from templates if needed
    template_vars = [
        {
            "module_name": "data_processor",
            "purpose": "Process and validate data entities",
            "class_name": "DataEntity",
            "entity_desc": "data entity",
            "processor_name": "DataProcessor",
        },
        {
            "module_name": "user_manager",
            "purpose": "Manage user accounts and sessions",
            "class_name": "UserAccount",
            "entity_desc": "user account",
            "processor_name": "UserManager",
        },
        {
            "module_name": "inventory_system",
            "purpose": "Track inventory items",
            "class_name": "InventoryItem",
            "entity_desc": "inventory item",
            "processor_name": "InventoryTracker",
        },
    ]

    template_questions = [
        {
            "question": "What happens when process_batch is called with an item that has an empty name?",
            "choices": [
                "A. The item is added successfully",
                "B. The item is skipped and logged as warning",
                "C. An exception is raised",
                "D. The program crashes"
            ],
            "answer": "B",
        },
        {
            "question": "What does the validate method check?",
            "choices": [
                "A. Only that the ID is not empty",
                "B. That ID and name are non-empty and value is non-negative",
                "C. Only that the value is positive",
                "D. That metadata contains required keys"
            ],
            "answer": "B",
        },
        {
            "question": "After calling clear(), what value does get_stats()['cleared'] return?",
            "choices": [
                "A. 0",
                "B. 1",
                "C. The number of items that were cleared",
                "D. None"
            ],
            "answer": "B",
        },
    ]

    for i, vars in enumerate(template_vars):
        if task_id >= num_tasks:
            break

        code = LONG_CODE_TEMPLATES["python"].format(**vars)
        q = template_questions[i % len(template_questions)]

        task = {
            "id": f"codeqa-{task_id:03d}",
            "question": q["question"],
            "code": code,
            "choices": q["choices"],
            "answer": q["answer"],
            "language": "python",
        }
        tasks.append(task)
        task_id += 1

    # Fill remaining with variations
    while task_id < num_tasks:
        # Cycle through existing snippets with slight modifications
        lang = random.choice(list(CODE_SNIPPETS.keys()))
        snippet = random.choice(CODE_SNIPPETS[lang])

        task = {
            "id": f"codeqa-{task_id:03d}",
            "question": snippet["question"],
            "code": snippet["code"],
            "choices": snippet["choices"],
            "answer": snippet["answer"],
            "language": lang,
        }
        tasks.append(task)
        task_id += 1

    # Write to JSONL
    with open(output_file, "w") as f:
        for task in tasks:
            f.write(json.dumps(task) + "\n")

    print(f"  Generated {len(tasks)} CodeQA tasks")


def convert_from_longbench(input_dir: Path, output_dir: Path) -> None:
    """Convert from LongBench v2 format."""
    output_file = output_dir / "codeqa.jsonl"

    # LongBench stores data in different possible locations
    possible_paths = [
        input_dir / "data" / "lcc.jsonl",  # Long Code Completion
        input_dir / "data" / "code_debug.jsonl",
        input_dir / "lcc.jsonl",
        input_dir / "code_debug.jsonl",
    ]

    source_file = None
    for path in possible_paths:
        if path.exists():
            source_file = path
            break

    if source_file:
        print(f"  Converting from {source_file}...")
        tasks = []
        task_id = 0

        with open(source_file) as f:
            for line in f:
                if task_id >= 50:  # Limit to 50 tasks
                    break

                data = json.loads(line)

                # LongBench format varies, adapt as needed
                task = {
                    "id": f"codeqa-{task_id:03d}",
                    "question": data.get("input", data.get("question", "")),
                    "code": data.get("context", data.get("code", "")),
                    "choices": data.get("choices", ["A. Option 1", "B. Option 2", "C. Option 3", "D. Option 4"]),
                    "answer": data.get("answers", [data.get("answer", "A")])[0] if isinstance(data.get("answers"), list) else data.get("answer", "A"),
                    "language": data.get("language", "python"),
                }
                tasks.append(task)
                task_id += 1

        with open(output_file, "w") as f:
            for task in tasks:
                f.write(json.dumps(task) + "\n")

        print(f"  Converted {len(tasks)} CodeQA tasks")
    else:
        print("  LongBench CodeQA files not found, generating synthetic...")
        generate_synthetic_tasks(output_file)


def main():
    if len(sys.argv) < 2:
        print("Usage: convert_codeqa.py <input_dir> <output_dir>")
        print("       convert_codeqa.py --synthetic <output_dir>")
        sys.exit(1)

    if sys.argv[1] == "--synthetic":
        output_dir = Path(sys.argv[2])
        output_dir.mkdir(parents=True, exist_ok=True)
        generate_synthetic_tasks(output_dir / "codeqa.jsonl")
    else:
        input_dir = Path(sys.argv[1])
        output_dir = Path(sys.argv[2])
        output_dir.mkdir(parents=True, exist_ok=True)
        convert_from_longbench(input_dir, output_dir)


if __name__ == "__main__":
    main()
