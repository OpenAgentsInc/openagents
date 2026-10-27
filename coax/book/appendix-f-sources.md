# Appendix F. Sources

## Coax

- [`coax/spec.md`](../spec.md): the specification and implementation plan
  (design of record).
- [`coax/grammar.ebnf`](../grammar.ebnf): the grammar `coax0` accepts.
- [`coax/README.md`](../README.md): building and using `coax0`.
- [The performance stack](https://github.com/OpenAgentsInc/aethyr-cpp/blob/main/docs/performance-stack.md)
  in aethyr-cpp: the engine plan Coax serves.

## Jai

The official compiler was not available to us. Everything about Jai here
comes from published material and public code. None of it is a normative
specification.

- BSVino, [The Jai Primer](https://github.com/BSVino/JaiPrimer/blob/master/JaiPrimer.md).
  An early outside summary of Blow's talks. Blow has said it doesn't
  describe the language well.
- Forrest Smith, [Advent of Code 2022 in Jai](https://github.com/forrestthewoods/aoc2022/tree/master/jai)
  (`main.jai`, `modules/autorun.jai`), and
  [Learning Jai via Advent of Code](https://www.forrestthewoods.com/blog/learning-jai-via-advent-of-code/) (2023).
- Forrest Smith, [Using Jai's Unique and Powerful Compiler for Typesafe Units](https://www.forrestthewoods.com/blog/using-jais-unique-and-powerful-compiler-for-typesafe-units/) (2024).
- Ivo Balbaert, [The Way to Jai](https://github.com/Ivo-Balbaert/The_Way_to_Jai):
  beta-user documentation for the compiler architecture, build system,
  context, metaprogramming, macros, arrays, strings and threads.
- Jonathan Blow, [Jai demo and design explanation, LambdaConf 2025](https://www.youtube.com/watch?v=IdpD5QIVOKQ):
  roughly 300,000 lines rebuilt from scratch in 2.3 seconds.
- Smári McCarthy, [Four Years of Jai](https://smarimccarthy.is/posts/2024-12-02-four-years-of-jai/) (2024):
  about 44,000 lines and three executables in about 1.3 seconds, over
  two-thirds of it in LLVM.
- Polygon, [Why I rewrote Polygon from scratch](https://polygonjs.com/blog/why-i-rewrote-polygon-from-scratch) (2026):
  a Jai engine on WebAssembly and WebGPU.
- Chip Collier, [Generating SDL3 bindings for Jai](https://allyourfaultforever.com/posts/jai-sdl3-bindings/) (2025).
- [Focus](https://github.com/focus-editor/focus), an editor written in Jai.
- Thekla / Arc Games: *Order of the Sinking Star* launched 8 October 2026;
  the 1 September 2026 announcement describes public compiler binaries,
  then source, after the game.
- *Jai: language, compiler, and engine architecture*, a technical research
  report dated 27 October 2026 (51 sources), shared by the owner. It
  shaped Chapters 1, 6, 9, 10, 13, 15 and 17, and Appendix C.

## Nearby languages

- [Odin](https://odin-lang.org/docs/overview/): implicit allocator context.
- [Zig](https://ziglang.org/): compile-time evaluation and explicit allocation.
- [Rust](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html):
  ownership as the alternative Coax does not take.
