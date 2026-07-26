//! Build script: hands the linker the architecture's memory layout.
//!
//! The link script cannot be named in `.cargo/config.toml`, because a relative
//! path there resolves against the directory cargo was invoked from rather than
//! against this crate. Doing it here makes the path absolute and correct however
//! the build was started.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");

    // Deliberately no `unwrap`/`expect`: cargo always sets both variables, and
    // if some future caller does not, silently declining to add a link argument
    // produces a clearer linker error than a panicking build script would.
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Ok(target) = std::env::var("TARGET") else {
        return;
    };

    let arch = if target.starts_with("aarch64") {
        "aarch64"
    } else if target.starts_with("x86_64") {
        // No link script yet: the x86_64 boot path is not implemented, and the
        // default layout is enough to link the placeholder entry point.
        return;
    } else {
        return;
    };

    let script = format!("{manifest_dir}/link/{arch}.ld");
    println!("cargo::rustc-link-arg=-T{script}");
    println!("cargo::rerun-if-changed={script}");
}
