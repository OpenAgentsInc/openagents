# 108. Choose the next example to learn from

> **Status: unimplemented — design draft.** Nothing here runs yet.

Labels cost money or time. Active learning lets a model pick which unlabeled
examples to label next. This chapter builds that loop on an owned pool and
asks one question: at the same label budget, does choosing data beat random
selection? A negative answer is a valid result.

## What this chapter covers

- An owned pool of examples with labels that an independent checker
  supplies. The selector never sees unqueried labels and never supplies its
  own labels.
- Three acquisition rules for a small classifier:
  - uncertainty (examples near the decision boundary);
  - diversity (examples unlike those already labeled);
  - a mixture of both.
- A random-selection baseline with the same trainer, the same initial
  labeled set and the same total label budget, including the initial labels.
- Several fixed seeds for every comparison.
- Held-out groups that stay outside acquisition and tuning.
- A confidently wrong region in the pool, which uncertainty alone can miss.
- A record of each query and its label, quality against labels acquired, and
  acquisition and training cost.

## Dependencies

- [Chapter 103](ch103-learning-with-arrays.md): the classifier and trainer.
- [Chapter 104](ch104-gradients-as-a-program.md): gradients.
- [Chapter 240](ch240-how-far-a-metric-moves-by-itself.md): noise floors for
  the comparison across seeds.
- No general framework from chapters 106–107 is needed.

## Done means

- For each rule, the report gives held-out quality against labels acquired,
  with the spread across seeds, beside random selection at equal budgets.
- The chapter states whether a rule beats random selection beyond the noise
  floor, or reports that it does not.
- The confidently wrong region is reported separately, with how many of its
  examples each rule queried.
- A check confirms that no held-out example and no unqueried label reached
  the selector.

## Sources

- [Chapter plan](PLAN.md)
