# HUD Rendering Fixes

## sRGB Color Space Gamma Correction Issue

**Date:** 2025-12-13
**Symptom:** Desktop (Metal) rendering showed washed out/desaturated colors compared to web (WebGL/WebGPU)

### Problem

When rendering syntax-highlighted text from syntect, the desktop version appeared significantly more desaturated than the web version, despite using the same code.

**Root Cause:**

Both platforms selected sRGB surface formats (via `.find(|f| f.is_srgb())`), but handled color conversion differently:

1. **syntect** provides colors in sRGB color space (perceptual, gamma ~2.2)
2. We stored these as HSLA (perceptually linear in L component)
3. We converted back to RGBA (still sRGB values 0-1)
4. We sent these to GPU shaders
5. **The issue:** sRGB surfaces expect *linear* RGB values and apply gamma correction automatically

On **desktop (Metal)**, the GPU was applying gamma correction to our already-gamma-corrected sRGB colors, resulting in double gamma correction → washed out appearance.

On **web (WebGL/WebGPU)**, the colors were not being gamma-corrected (or handled differently), so they appeared correct.

### Solution

Added `to_linear_rgba()` method to convert sRGB colors to linear RGB, but **only use it on desktop** (not WASM/web):

**File:** `crates/wgpui/src/color.rs`
```rust
/// Convert to linear RGB space for sRGB surfaces.
/// sRGB uses gamma ~2.2, so we need to raise to power 2.4 to convert to linear.
pub fn to_linear_rgba(&self) -> [f32; 4] {
    let rgba = self.to_rgba();
    [
        Self::srgb_to_linear(rgba[0]),
        Self::srgb_to_linear(rgba[1]),
        Self::srgb_to_linear(rgba[2]),
        rgba[3], // alpha is always linear
    ]
}

/// Convert a single sRGB component to linear.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
```

**File:** `crates/wgpui/src/scene.rs`
```rust
// For text rendering - platform specific!
color: {
    #[cfg(not(target_arch = "wasm32"))]
    { color.to_linear_rgba() } // Desktop: Convert to linear for Metal's sRGB handling
    #[cfg(target_arch = "wasm32")]
    { color.to_rgba() } // Web: Keep as sRGB (WebGL/WebGPU don't auto-convert)
}

// For quad backgrounds - platform specific!
background: quad.background.map(|c| {
    #[cfg(not(target_arch = "wasm32"))]
    { c.to_linear_rgba() } // Desktop: Convert to linear
    #[cfg(target_arch = "wasm32")]
    { c.to_rgba() } // Web: Keep as sRGB
}).unwrap_or([0.0, 0.0, 0.0, 0.0]),

// For borders - platform specific!
border_color: {
    #[cfg(not(target_arch = "wasm32"))]
    { quad.border_color.to_linear_rgba() } // Desktop: Convert to linear
    #[cfg(target_arch = "wasm32")]
    { quad.border_color.to_rgba() } // Web: Keep as sRGB
}
```

### Technical Details

**sRGB to Linear conversion formula:**
- For c ≤ 0.04045: `linear = c / 12.92`
- For c > 0.04045: `linear = ((c + 0.055) / 1.055)^2.4`

This is the standard sRGB EOTF (Electro-Optical Transfer Function) defined in IEC 61966-2-1.

**Why this matters:**
- sRGB surface formats *should* automatically apply gamma correction when writing colors
- **Desktop (Metal on macOS):** sRGB surfaces DO auto-convert linear → sRGB for display
  - Give it linear RGB: correct colors ✅
  - Give it sRGB: double gamma correction, washed out ❌
- **Web (WebGL/WebGPU):** sRGB surfaces do NOT auto-convert
  - Give it linear RGB: too dark (missing gamma correction) ❌
  - Give it sRGB: correct colors ✅
- Human vision is nonlinear, so proper gamma handling is critical for perceived color accuracy
- **Platform differences are real:** Always test cross-platform when dealing with color!

### Prevention

When working with GPU rendering:
1. **Always know your color space:** Are your input colors sRGB or linear?
2. **Match surface expectations:** sRGB surfaces expect linear input
3. **Document conversions:** Make it explicit where conversions happen
4. **Test cross-platform:** Metal, Vulkan, WebGL, and WebGPU may handle this differently

### Related Code

- Color conversion: `crates/wgpui/src/color.rs`
- Scene GPU data: `crates/wgpui/src/scene.rs`
- Syntax highlighting: `crates/wgpui/src/markdown/highlighter.rs`
- Surface creation: `crates/wgpui/src/platform/{desktop.rs,web.rs}`
- Shaders: `crates/wgpui/src/shaders/{quad.wgsl,text.wgsl}`

### References

- [sRGB Wikipedia](https://en.wikipedia.org/wiki/SRGB)
- [GPU Gems 3: The Importance of Being Linear](https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-24-importance-being-linear)
- [wgpu TextureFormat docs](https://docs.rs/wgpu/latest/wgpu/enum.TextureFormat.html)
