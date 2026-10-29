# Psionic original layer mapping verification, 2026-10-29

The layer mapper binds the exact four-layer hybrid schedule from a separately
authenticated original inventory. It checks names, BF16 shapes and vector
values; matrices remain borrowed from original files. All 32 Elephant layers
(24 linear and 8 full-attention) mapped from the pinned original inventory.

On the development Mac, 131 Coax checks passed in debug and release; all
3,486 mutation candidates were killed. Native debug and release checks passed
normally and with ASan/UBSan. Native release checks passed on CoderOS. The
native adapter observes nested overlaps; valid storage, lifetime, exclusive
access and prior authentication remain caller obligations.
