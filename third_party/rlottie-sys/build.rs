use std::{env, path::PathBuf};

#[cfg(feature = "vendor-samsung")]
mod source {
	pub const VENDOR: &str = "samsung";
	pub const GIT_REPO: &str = "https://github.com/Samsung/rlottie.git";
	pub const GIT_REV: &str = "v0.2";
	pub const GIT_PATCHES: &[&str] = &["2d7b1fa2b005bba3d4b45e8ebfa632060e8a157a"];
}

#[cfg(all(not(feature = "vendor-samsung"), feature = "vendor-telegram"))]
mod source {
	pub const VENDOR: &str = "telegram";
	pub const GIT_REPO: &str = "https://github.com/msrd0/rlottie-telegram";
	pub const GIT_REV: &str = "f435c0a4f364ce39f7e4eee9ddc7118573da9bf9";
	pub const GIT_PATCHES: &[&str] = &[];
}

#[cfg(feature = "__vendor")]
fn compile_rlottie() -> Vec<PathBuf> {
	use crate::source::*;
	use std::{
		env::current_dir,
		fmt::{self, Display, Formatter},
		fs::{create_dir_all, read_to_string, write},
		process::{Command, Stdio}
	};

	struct DisplayCommand<'a>(&'a Command);

	impl Display for DisplayCommand<'_> {
		fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
			write!(f, "{:?}", self.0.get_program())?;
			for arg in self.0.get_args() {
				write!(f, " {arg:?}")?;
			}
			Ok(())
		}
	}

	fn print_cmd(cmd: &Command) {
		println!(
			"{} $ {}",
			cmd.get_current_dir()
				.map(PathBuf::from)
				.unwrap_or_else(|| current_dir().unwrap())
				.display(),
			DisplayCommand(cmd)
		);
	}

	fn try_run(cmd: &mut Command) -> bool {
		// no user interactions, ever
		cmd.stdin(Stdio::null());

		print_cmd(cmd);
		cmd.status().is_ok_and(|status| status.success())
	}

	fn run(cmd: &mut Command) {
		// no user interactions, ever
		cmd.stdin(Stdio::null());

		print_cmd(cmd);
		match cmd.status() {
			Ok(status) if status.success() => {},
			Ok(status) => panic!(
				"Failed to run {}: Exit code {:?}",
				DisplayCommand(cmd),
				status.code()
			),
			Err(err) => panic!("Failed to run {}: {err:?}", DisplayCommand(cmd))
		}
	}

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let src_dir = out_dir.join(format!("rlottie-{VENDOR}-{GIT_REV}"));
	let build_dir = out_dir.join(format!("build-{VENDOR}-{GIT_REV}"));

	if !src_dir.exists() {
		run(Command::new("git")
			.arg("clone")
			.arg(GIT_REPO)
			.arg(&src_dir)
			.current_dir(&out_dir));
		// since telegram doesn't merge fixes we need to download pull requests
		if VENDOR == "telegram" {
			run(Command::new("git")
				.arg("config")
				.arg("--add")
				.arg("remote.origin.fetch")
				.arg("refs/pull/*/head:refs/remotes/origin/pull/*")
				.current_dir(&src_dir));
			run(Command::new("git").arg("fetch").current_dir(&src_dir));
		}
	}
	run(Command::new("git")
		.arg("checkout")
		.arg(GIT_REV)
		.current_dir(&src_dir));
	for rev in GIT_PATCHES {
		run(Command::new("git")
			.arg("cherry-pick")
			.arg("-x")
			.arg(rev)
			.current_dir(&src_dir)
			.env("GIT_COMMITTER_NAME", "nobody")
			.env("GIT_COMMITTER_EMAIL", "nobody"));
	}

	if VENDOR == "samsung" {
		let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
		let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
		if target_os == "macos" && target_arch == "aarch64" {
			let vector_cmake = src_dir.join("src/vector/CMakeLists.txt");
			let vector_cmake_source =
				read_to_string(&vector_cmake).expect("Failed to read rlottie vector CMakeLists.txt");
			let neon_entry = "        \"${CMAKE_CURRENT_LIST_DIR}/vdrawhelper_neon.cpp\"\n";
			let vector_cmake_patched = if vector_cmake_source.contains(neon_entry) {
				vector_cmake_source
			} else {
				vector_cmake_source.replace(
					"        \"${CMAKE_CURRENT_LIST_DIR}/vdrawhelper_sse2.cpp\"\n",
					&format!(
						"        \"${{CMAKE_CURRENT_LIST_DIR}}/vdrawhelper_sse2.cpp\"\n{neon_entry}"
					),
				)
			};
			write(&vector_cmake, vector_cmake_patched)
				.expect("Failed to patch rlottie vector CMakeLists.txt");

			let neon_cpp = src_dir.join("src/vector/vdrawhelper_neon.cpp");
			write(
				&neon_cpp,
				r#"/*
 * Apple Silicon fallback: keep the NEON translation unit present so rlottie
 * still links, but use the generic fill path instead of Samsung's pixman asm.
 */

#include "vdrawhelper.h"

void memfill32(uint32_t *dest, uint32_t value, int length)
{
    for (int i = 0; i < length; ++i) {
        dest[i] = value;
    }
}

void RenderFuncTable::neon() {}
"#,
			)
			.expect("Failed to patch rlottie NEON helper");
		}
	}

	// we could revisit this if the `cmake` crate becomes less opinionated
	//  - `cmake` forces setting of `CMAKE_BUILD_TYPE` which is fine unless you are part
	//    of an already build-tool-independent configured environment like a linux
	//    distribution packaging process where they want the same flags for everything
	//  - `cmake` forces setting of `CMAKE_INSTALL_PREFIX` for no good reason
	//  - `cmake` has no way to set the build directory, and it doesn't use out_dir
	//    as its target directory
	//  - `cmake` prints some weird `cargo:root` line that I only found mentioned here
	//    https://web.mit.edu/rust-lang_v1.25/arch/amd64_ubuntu1404/share/doc/rust/html/cargo/reference/build-scripts.html#outputs-of-the-build-script
	//    where it is called "user-defined metadata" but cmake is only part of the build
	//    script so who would use that metadata?
	// cmake::Config::new(&src_dir)
	// 	.out_dir(&build_dir)
	// 	.build_target("rlottie")
	// 	.build();

	// for the time being, I force my own opinions on how to use cmake
	create_dir_all(&build_dir).expect("Failed to create directory");
	let mut cmake = Command::new("cmake");
	cmake.arg("-Wno-dev");
	cmake.arg("-DCMAKE_POLICY_VERSION_MINIMUM=3.5");
	cmake.arg("-DBUILD_SHARED_LIBS=OFF");
	if try_run(Command::new("which").arg("ninja")) {
		cmake.arg("-G").arg("Ninja");
	}
	if env::var("CFLAGS").is_err() {
		let opt_level = env::var("OPT_LEVEL").unwrap();
		let mut build_type = "Release";
		// TODO rust supports debug info in release builds, we should detect that
		if opt_level == "0" || opt_level == "g" {
			build_type = "RelWithDebInfo";
		} else if opt_level == "s" || opt_level == "z" {
			build_type = "MinSizeRel";
		}
		cmake.arg(format!("-DCMAKE_BUILD_TYPE={build_type}"));
	}
	cmake.arg(&src_dir);
	cmake.current_dir(&build_dir);
	run(&mut cmake);
	run(Command::new("cmake")
		.arg("--build")
		.arg(".")
		.arg("--target")
		.arg("rlottie")
		.current_dir(&build_dir));

	println!("cargo:rustc-link-search=native={}", build_dir.display());
	println!("cargo:rustc-link-lib=static=rlottie");
	// Samsung rlottie is C++, but macOS links libc++ rather than libstdc++.
	if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
		println!("cargo:rustc-link-lib=c++");
	} else {
		println!("cargo:rustc-link-lib=stdc++");
	}

	vec![src_dir.join("inc")]
}

fn main() {
	let include_paths = pkg_config::Config::new()
		.probe("rlottie")
		.map(|lib| lib.include_paths);

	#[cfg(not(feature = "__vendor"))]
	let include_paths = include_path.expect("Unable to find rlottie");

	#[cfg(feature = "__vendor")]
	let include_paths = include_paths.unwrap_or_else(|_| compile_rlottie());

	println!("cargo:rerun-if-changed=wrapper.h");
	let bindings = bindgen::Builder::default()
		.formatter(bindgen::Formatter::Prettyplease)
		.clang_args(
			include_paths
				.into_iter()
				.map(|path| format!("-I{}", path.display()))
		)
		.header("wrapper.h")
		.parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
		.newtype_enum(".*")
		.rust_target(bindgen::RustTarget::stable(70, 0).unwrap_or_else(|_| panic!()))
		.size_t_is_usize(true)
		.use_core()
		.generate()
		.expect("Unable to generate bindings");

	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	bindings
		.write_to_file(out_dir.join("bindings.rs"))
		.expect("Couldn't write bindings!");
}
