---
id: environment.lazy-library-state-and-custom-allocator-lifetimes
version: 1
kind: edge-case
title: Warm up lazy library state before activating a temporary custom allocator
summary: >-
  Libraries may lazily allocate locale, formatting, or other process-lifetime
  state on first use. If a custom allocator or arena is active only
  temporarily, trigger such initialization before it starts and release
  library-owned pools only through a supported lifecycle hook.
tags: 
  - c++
  - allocator
  - lifetime
  - locale
  - valgrind
applies_when: >-
  C++ programs combine custom or temporary heaps with libraries that lazily
  initialize global or static state, especially when teardown or exit-time
  destructors touch those allocations.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - custom-memory-heap-crash
  cites:
    - ISO/IEC 14882:2020, Programming Languages — C++, clauses [basic.start.init] and [basic.start.term]
    - GNU libstdc++ documentation, The GNU C++ Library Manual, Locales
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Lazy initialization can make allocator lifetime order observable. A first-use operation such as formatted stream output may create locale facets or other global library state. If that initialization happens while a short-lived custom heap is active, later static destruction can access freed storage. Arrange initialization before switching to the temporary allocator, or ensure allocations use an allocator whose lifetime spans all consumers and destructors.

Cleanup is a separate concern: library-owned caches or emergency pools may remain reachable until process exit. Do not call guessed or private cleanup symbols merely to satisfy leak reports; use documented lifecycle APIs where available, and distinguish intentional reachable storage from invalid access or genuinely lost allocations.

Source: ISO/IEC 14882:2020, *Programming Languages — C++*, clauses `[basic.start.init]` and `[basic.start.term]` (dynamic initialization and termination sequencing); GNU libstdc++ documentation, *The GNU C++ Library Manual*, "Locales" (locale facets and locale use).
