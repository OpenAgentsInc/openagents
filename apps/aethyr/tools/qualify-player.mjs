/** Built player qualification. Build with VITE_PLAYER_DIAGNOSTICS=1 first. */
process.env.AF_PLAYER_PERFORMANCE = "1";
process.env.AF_PERFORMANCE_MODE ??= "qualification";
await import("./verify-player.mjs");
