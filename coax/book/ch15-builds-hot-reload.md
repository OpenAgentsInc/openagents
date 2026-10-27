# 15. Builds and hot reload

> **Design.** Spec §16. `coax0` is a single-shot compiler driven by the
> command line. The build program, the resident compiler and reload are
> later stages.

## 15.1 The build is a program

```coax
#import "Build";

build :: () {
  w := Build.workspace("verse");
  w.options.targets = .[.macos_arm64, .ios_arm64];
  Build.add_files(w, "src/**/*.coax");
  Build.add_c_library(w, "third_party/stb", headers = "stb_image.h");
  Build.emit_xcode_sources(w, "apps/openagents-ios");
}
```

In Jai the build is a Jai program: the compiler starts a metaprogram that
creates workspaces, sets options and handles compiler messages. Coax
does the same with `build.coax`, and adds what the Jai report flagged as
missing.

**Capabilities.** Build code runs on the build machine, separate from
target code. Reading files, running processes or touching the network
requires a declared permission. Pure `#run` code never inherits it.

**A recorded set of inputs.** A reproducible build records:

- the compiler version and hash;
- the target description;
- toolchain versions;
- source, module and plugin hashes;
- options;
- every file it read.

Directory listings are sorted, environment reads are allowlisted, and
clocks and randomness are absent by default. Network inputs must be
pinned, fetched artifacts with explicit opt-in.

**Not a package manager.** A programmable build doesn't come with
dependency resolution, lockfiles, remote caching, hermetic execution or
artifact signing. Each of those is its own system.

**Generate before you replace.** `build.coax` will first *generate* the
CMake and Xcode source lists. The existing CMake, `project.yml`, Xcode
and Emscripten paths stay until generated output matches them and real
native and browser builds pass.

## 15.2 Measure the whole loop

Compiler speed is one stage of the loop you actually wait on. A
measurement has to separate:

1. parsing, checking and compile-time evaluation;
2. code generation, optimization and linking;
3. asset processing;
4. launch, GPU pipeline preparation, or reload;
5. for the web: Wasm processing, compression, transfer, instantiation.

Measure on named hardware, at a named revision, more than once, and
report the workload's size. Jai's best public numbers are clean rebuilds
of a single project, and a third of one reported build was front end
while two thirds was LLVM. The number that matters is **edit to visible
result**.

## 15.3 Hot reload is an engine design

A fast compiler makes hot reload attractive. It doesn't make it safe.
Reload needs all of these, and none come from compile speed:

- **A resident host** that owns persistent storage, platform services,
  queues and GPU resources, and never reloads.
- **A versioned C interface** to the reloadable code.
- **A safe point.** Workers stop, and no stack frame or callback from the
  old code can still run.
- **Stable procedure IDs.** Stored references to code are IDs into a
  table, never raw addresses that go stale.
- **Transactional migration.** If a persistent struct's layout changed:
  allocate new storage, migrate field by field with checks, validate
  handles and resources, then publish atomically, or roll back and keep
  the old state. Compatibility is never inferred from matching field
  names.
- **GPU resources that survive**, with pipelines replaced between frames.

## 15.4 iOS doesn't load code

iOS doesn't allow loading new code into a signed app at run time, and
Coax doesn't try to get around that. Development reload runs on a Mac
host, where the engine already runs natively on Metal. Device builds are
fixed, signed images.
