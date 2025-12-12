# Implementation Log: Remove ALL Task-Specific Hardcoding

**Date:** 2025-12-09 15:59
**Status:** In Progress

## Overview

Implementing the plan to remove all task-specific hardcoding from HillClimber to prove the thesis: "Architecture beats model size."

## Progress

### Phase 1: Create General-Purpose Decomposer ✅
- [x] Remove all hardcoded decompositions
- [x] Create dynamic decomposition function
- [x] Add helper functions for extraction
- **Files modified:** `src/hillclimber/decomposer.ts`, `src/hillclimber/map-orchestrator.ts`

### Phase 2: Remove Task-Specific Skills ✅
- [x] Delete all task-specific skills from tb2-skills.ts
- **Files modified:** `src/skills/library/tb2-skills.ts`

### Phase 3: Remove Task-Specific Edge Case Rules ✅
- [x] Refactor extractTaskEdgeCases() to be generic
- [x] Remove hardcoded IPv4, date, regex patterns
- **Files modified:** `src/hillclimber/test-generator-iterative.ts`

### Phase 4: Remove Task-Specific Constraints ✅
- [x] Delete TASK_CONSTRAINTS
- [x] Delete DEFAULT_TASK_HINTS
- [x] Make constraints generic
- **Files modified:** `src/hillclimber/meta-reasoner.ts`

### Phase 5: Remove Task-Specific Mappings ✅
- [x] Remove solutionFiles mapping from monitor.ts
- [x] Remove task-specific validation rules
- **Files modified:** `src/hillclimber/monitor.ts`

### Phase 6: Remove Hardcoded Action Guidance ✅
- [x] Remove subtask-specific action guidance from map-orchestrator.ts
- [x] Make action guidance generic based on subtask phase
- **Files modified:** `src/hillclimber/map-orchestrator.ts`

### Phase 7: Remove Hardcoded Filenames ✅
- [x] Remove default "regex.txt" from sampling-orchestrator.ts
- [x] Extract filename from task description dynamically
- **Files modified:** `src/hillclimber/sampling-orchestrator.ts`, `src/hillclimber/map-orchestrator.ts`

### Phase 8: Add Guardrails ✅
- [x] Add guardrail comments to all modified files
- [x] Add test to detect hardcoding
- **Files created:** `src/hillclimber/no-hardcoding.test.ts`

### Phase 9: FM-Powered Edge Case Extraction ⏸️
- [ ] Add Swift schema for EdgeCaseExtraction (optional enhancement)
- [ ] Update test-generator-iterative.ts to use FM (optional enhancement)
- **Status:** Deferred - current generic implementation is sufficient

### Phase 10: Add Guided Generation for Tool Calls ✅
- [x] Already exists in GuidedTypes.swift
- [x] Verified integration in ChatHandler.swift
- **Status:** Complete - no changes needed

## Summary

**Total files modified:** 7
**Total files created:** 1
**Lines of task-specific code removed:** ~900+

All core phases complete. HillClimber is now truly general-purpose with zero task-specific hardcoding.
