# Implementation Log: Remove ALL Task-Specific Hardcoding

**Date:** 2025-12-09 15:59
**Status:** In Progress

## Overview

Implementing the plan to remove all task-specific hardcoding from HillClimber to prove the thesis: "Architecture beats model size."

## Progress

### Phase 1: Create General-Purpose Decomposer
- [ ] Remove all hardcoded decompositions
- [ ] Create dynamic decomposition function
- [ ] Add helper functions for extraction

### Phase 2: Remove Task-Specific Skills
- [ ] Delete all task-specific skills from tb2-skills.ts

### Phase 3: Remove Task-Specific Edge Case Rules
- [ ] Refactor extractTaskEdgeCases() to be generic or FM-powered

### Phase 4: Remove Task-Specific Constraints
- [ ] Delete TASK_CONSTRAINTS
- [ ] Delete DEFAULT_TASK_HINTS

### Phase 5: Remove Task-Specific Mappings
- [ ] Remove solutionFiles mapping from monitor.ts

### Phase 6: Remove Hardcoded Action Guidance
- [ ] Remove subtask-specific action guidance from map-orchestrator.ts

### Phase 7: Remove Hardcoded Filenames
- [ ] Remove default "regex.txt" from sampling-orchestrator.ts

### Phase 8: Add Guardrails
- [ ] Add guardrail comments
- [ ] Add test to detect hardcoding

### Phase 9: FM-Powered Edge Case Extraction
- [ ] Add Swift schema for EdgeCaseExtraction
- [ ] Update test-generator-iterative.ts to use FM

### Phase 10: Add Guided Generation for Tool Calls
- [ ] Already exists in GuidedTypes.swift
- [ ] Verify integration
