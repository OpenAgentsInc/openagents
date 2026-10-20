//! The two fragment shaders, as complete GLSL ES 3.00 sources, and the
//! minifier the site runs before it inlines them.
//!
//! The passes are written the Shader Skills way: Shadertoy's
//! `mainImage(out vec4, in vec2)` with `iResolution`, `iTime`, `iFrame` and
//! `iChannel0`, calling the library's functions by name. [`PRELUDE`]
//! declares those uniforms and [`MAIN`] calls `mainImage`, so each pass is
//! `PRELUDE + library + scene + pass + MAIN`, the same unit the skill's
//! checker compiles.

/// The Shader Skills functions both passes call (MIT, gokayfem), bundled
/// by the skill's `scripts/glsl_tools.py`; see the crate README.
pub const LIBRARY: &str = include_str!("../shaders/library.glsl");
/// Scene constants and the sky table's mapping, shared by both passes.
pub const SCENE: &str = include_str!("../shaders/scene.glsl");
/// Pass 1: fills the sky-view table.
pub const SKY: &str = include_str!("../shaders/sky.glsl");
/// Pass 2: the Grid, every frame.
pub const GRID: &str = include_str!("../shaders/grid.glsl");

/// The Shadertoy inputs the passes read, as WebGL2 uniforms.
pub const PRELUDE: &str = "#version 300 es
precision highp float;
precision highp int;
uniform vec3 iResolution;
uniform float iTime;
uniform int iFrame;
uniform sampler2D iChannel0;
out vec4 shaderColor;
";

/// Calls the pass's `mainImage` for the fragment.
pub const MAIN: &str =
    "\nvoid main() { mainImage(shaderColor, gl_FragCoord.xy); shaderColor.a = 1.0; }\n";

/// The full-screen triangle every pass draws, with no vertex buffer.
pub const VERTEX: &str = "#version 300 es
void main() { vec2 p = vec2(gl_VertexID & 1, gl_VertexID >> 1) * 4.0 - 1.0; gl_Position = vec4(p, 0.0, 1.0); }
";

/// The page's elements the runner reads: the canvas and the two sources.
pub const CANVAS_ID: &str = "lander5-canvas";
pub const SKY_ID: &str = "lander5-sky";
pub const GRID_ID: &str = "lander5-grid";

/// The sky table's size in texels (azimuth, elevation).
pub const SKY_TABLE_SIZE: (i32, i32) = (128, 128);

/// The library's license and credit (its leading comment), as text. The
/// minified passes carry no comments, so the page shows this beside them.
#[must_use]
pub fn library_notice() -> String {
    let mut notice = String::new();
    for line in LIBRARY.lines() {
        let Some(text) = line.strip_prefix("//") else {
            break;
        };
        notice.push_str(text.strip_prefix(' ').unwrap_or(text));
        notice.push('\n');
    }
    notice
}

/// The sky pass, complete.
#[must_use]
pub fn sky_source() -> String {
    [PRELUDE, LIBRARY, SCENE, SKY, MAIN].concat()
}

/// The grid pass, complete.
#[must_use]
pub fn grid_source() -> String {
    [PRELUDE, LIBRARY, SCENE, GRID, MAIN].concat()
}

/// GLSL without comments, indentation, or blank lines. Every line break
/// stays, so preprocessor lines keep a line of their own. GLSL has no
/// string literals, so every `//` and `/*` starts a comment.
#[must_use]
pub fn minify(source: &str) -> String {
    let mut without = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("/*") {
        without.push_str(&rest[..start]);
        let comment_end = rest[start + 2..]
            .find("*/")
            .map_or(rest.len(), |end| start + 2 + end + 2);
        // The comment's line breaks stay, so every line keeps its place.
        for _ in rest[start..comment_end].matches('\n') {
            without.push('\n');
        }
        rest = &rest[comment_end..];
    }
    without.push_str(rest);
    let mut out = String::with_capacity(without.len());
    for line in without.lines() {
        let code = line.split("//").next().unwrap_or_default();
        let code = code.split_whitespace().collect::<Vec<_>>().join(" ");
        if !code.is_empty() {
            out.push_str(&code);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_pass_is_a_complete_unit() {
        for source in [sky_source(), grid_source()] {
            assert!(source.starts_with("#version 300 es\n"));
            assert_eq!(source.matches("#version").count(), 1);
            assert_eq!(source.matches("void mainImage(").count(), 1);
            assert!(source.contains("void main()"));
        }
        assert!(grid_source().contains("#define DEBUG_MODE 0"));
    }

    #[test]
    fn the_library_keeps_its_license_and_credit() {
        assert!(LIBRARY.contains("Copyright (c) 2026 gokayfem"));
        assert!(LIBRARY.contains("https://github.com/gokayfem/shader-skills"));
        assert!(LIBRARY.contains("Permission is hereby granted, free of charge"));
        let notice = library_notice();
        assert!(notice.starts_with("Building blocks from Shader Skills by gokayfem\n"));
        assert!(notice.contains("Copyright (c) 2026 gokayfem"));
        assert!(notice.trim_end().ends_with("SOFTWARE."));
        assert!(!notice.contains("--"), "an HTML comment can't hold it");
    }

    /// The skill's hard rules: no sine hash, no WebGL1 names, no `#version`
    /// in a pass.
    #[test]
    fn the_sources_follow_the_skill_rules() {
        for source in [LIBRARY, SCENE, SKY, GRID] {
            let code = minify(source);
            assert!(!code.contains("43758.5453"));
            assert!(!code.contains("gl_FragColor"));
            assert!(!code.contains("texture2D"));
            assert!(!code.contains("#version"));
        }
    }

    #[test]
    fn minify_drops_comments_and_keeps_directives_on_their_own_lines() {
        let source = "// a\n#define A 1 // b\n  float x = 1.0; /* c\n d */ float y;\n\n";
        assert_eq!(minify(source), "#define A 1\nfloat x = 1.0;\nfloat y;\n");
        let grid = minify(&grid_source());
        assert!(grid.len() * 5 < grid_source().len() * 3, "{}", grid.len());
        assert!(grid.lines().any(|line| line == "#define DEBUG_MODE 0"));
        assert!(!grid.contains("//"));
    }

    /// The minified passes compile with glslangValidator, the checker the
    /// skill uses, when it is installed (`brew install glslang`).
    #[test]
    fn the_minified_passes_compile_where_glslang_is_installed() {
        for (name, source) in [("sky", sky_source()), ("grid", grid_source())] {
            let path =
                std::env::temp_dir().join(format!("lander5-{name}-{}.frag", std::process::id()));
            std::fs::write(&path, minify(&source)).unwrap();
            let result = std::process::Command::new("glslangValidator")
                .arg(&path)
                .output();
            let _ = std::fs::remove_file(&path);
            match result {
                Ok(out) => assert!(
                    out.status.success(),
                    "{name}: {}",
                    String::from_utf8_lossy(&out.stdout)
                ),
                Err(_) => eprintln!("glslangValidator is not installed; {name} was not compiled"),
            }
        }
    }
}
