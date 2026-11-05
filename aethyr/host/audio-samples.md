# SDK PCM publication

`hdo_finite(bytes)` admits complete little-endian F32 words through the
already checked companion finite-word predicate. Signed zeros, subnormals
and finite extremes are preserved; NaN, infinity and partial words refuse.
Shape and allocation admission remain separate. The empty view is finite.
Its five root mutants are killed, without equivalents.
