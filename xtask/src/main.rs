// SPDX-License-Identifier: GPL-3.0-or-later

//! Build, run and boot-test automation for the Setonix kernel.
//!
//! Invoked as `cargo xtask <command>`, via the alias in `.cargo/config.toml`.
//!
//! ```text
//! cargo xtask build     --arch aarch64 [--release]
//! cargo xtask run-qemu  --arch aarch64 [--release] [--debug]
//! cargo xtask boot-test --arch aarch64 --expect "Kaya!" [--timeout 30]
//! ```
//!
//! `boot-test` is the one CI depends on: it boots the kernel under QEMU, watches
//! the serial console for an expected string, and fails if it does not appear
//! before the deadline. That single check exercises the link script, the boot
//! stub, the stack, `.bss` zeroing, the hardware-abstraction boundary and the
//! console device together — which is why it is worth more than any unit test
//! available at this stage.

use std::{
    env,
    error::Error,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Shorthand for a fallible operation with a human-readable error.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// A Tier-1 architecture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Arch {
    /// The lead architecture in the bring-up order.
    Aarch64,
    /// The second Tier-1 architecture; boot path not yet implemented.
    X86_64,
}

impl Arch {
    /// Parses an architecture name as given on the command line.
    fn parse(name: &str) -> Result<Self> {
        match name {
            "aarch64" | "arm64" => Ok(Self::Aarch64),
            "x86_64" | "amd64" => Ok(Self::X86_64),
            other => Err(format!(
                "unknown architecture '{other}' (expected 'aarch64' or 'x86_64')"
            )
            .into()),
        }
    }

    /// The Rust target triple.
    ///
    /// Both are soft-float. `x86_64-unknown-none` already is; AArch64 needs the
    /// explicit `-softfloat` variant, because the plain one permits NEON and the
    /// kernel would then trap on `CPACR_EL1.FPEN` before reaching its console.
    /// See `rust-toolchain.toml` for the full reasoning.
    const fn triple(self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64-unknown-none-softfloat",
            Self::X86_64 => "x86_64-unknown-none",
        }
    }

    /// The QEMU system emulator binary.
    const fn qemu(self) -> &'static str {
        match self {
            Self::Aarch64 => "qemu-system-aarch64",
            Self::X86_64 => "qemu-system-x86_64",
        }
    }

    /// Machine arguments common to running and boot-testing.
    fn machine_args(self) -> Vec<&'static str> {
        match self {
            Self::Aarch64 => vec![
                "-machine",
                "virt",
                "-cpu",
                "cortex-a72",
                "-smp",
                "1",
                "-m",
                "512M",
            ],
            Self::X86_64 => vec!["-machine", "q35", "-smp", "1", "-m", "512M"],
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

/// Parses the command line and dispatches.
fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first() else {
        print_usage();
        return Err("no command given".into());
    };

    let arch = match flag_value(&args, "--arch") {
        Some(value) => Arch::parse(&value)?,
        // Defaulting to the lead architecture keeps the common case short.
        None => Arch::Aarch64,
    };
    let release = args.iter().any(|a| a == "--release");
    let features = flag_value(&args, "--features");

    match command.as_str() {
        "build" => build(arch, release, features.as_deref()).map(|image| {
            println!("xtask: {}", image.display());
        }),
        "run-qemu" => run_qemu(
            arch,
            release,
            args.iter().any(|a| a == "--debug"),
            features.as_deref(),
        ),
        "boot-test" => {
            let expect =
                flag_value(&args, "--expect").ok_or("boot-test requires --expect <string>")?;
            let timeout = match flag_value(&args, "--timeout") {
                Some(value) => Duration::from_secs(value.parse::<u64>()?),
                None => Duration::from_secs(30),
            };
            boot_test(arch, release, &expect, timeout, features.as_deref())
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => {
            print_usage();
            Err(format!("unknown command '{other}'").into())
        }
    }
}

/// Prints usage to stdout.
fn print_usage() {
    println!(
        "\
cargo xtask <command> [options]

Commands:
  build       Cross-compile the kernel
  run-qemu    Boot the kernel under QEMU, serial attached to this terminal
  boot-test   Boot under QEMU and require an expected string on the console
  help        Show this message

Options:
  --arch <aarch64|x86_64>   Target architecture (default: aarch64)
  --release                 Build with optimisations
  --features <list>         Kernel cargo features, comma-separated
                            (e.g. provoke-exception, for the exception self-test)
  --debug                   run-qemu only: halt at reset and await gdb on :1234
  --expect <string>         boot-test only: the string that must appear
  --timeout <seconds>       boot-test only: deadline (default: 30)"
    );
}

/// Returns the value following `flag`, if present.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let index = args.iter().position(|a| a == flag)?;
    args.get(index + 1).cloned()
}

/// The workspace root, derived from this crate's location rather than the
/// current directory, so that `cargo xtask` behaves the same from anywhere.
fn workspace_root() -> Result<PathBuf> {
    let manifest = env::var("CARGO_MANIFEST_DIR")?;
    Path::new(&manifest)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot locate workspace root from xtask manifest".into())
}

/// The cargo binary that invoked us, so the pinned toolchain is preserved.
fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Cross-compiles the kernel and returns the path to the resulting image.
fn build(arch: Arch, release: bool, features: Option<&str>) -> Result<PathBuf> {
    let root = workspace_root()?;

    let mut command = Command::new(cargo());
    command.current_dir(&root).args([
        "build",
        "--package",
        "setonix-kernel",
        "--target",
        arch.triple(),
    ]);
    if release {
        command.arg("--release");
    }
    if let Some(features) = features {
        command.args(["--features", features]);
    }

    println!("xtask: building setonix-kernel for {}", arch.triple());
    let status = command.status()?;
    if !status.success() {
        return Err(format!("cargo build failed with {status}").into());
    }

    let profile = if release { "release" } else { "debug" };
    let image = root
        .join("target")
        .join(arch.triple())
        .join(profile)
        .join("setonix-kernel");

    if !image.exists() {
        return Err(format!("expected image at {} but it is missing", image.display()).into());
    }
    Ok(image)
}

/// Rejects architectures whose boot path is not implemented yet.
fn require_bootable(arch: Arch) -> Result<()> {
    if arch == Arch::X86_64 {
        return Err("the x86_64 boot path is not implemented yet — \
                    q35 has no bare-ELF equivalent of -kernel, so this needs a \
                    UEFI stub and an ESP image first (see kernel/src/arch/x86_64/mod.rs)"
            .into());
    }
    Ok(())
}

/// Boots the kernel with the serial console attached to this terminal.
fn run_qemu(arch: Arch, release: bool, debug: bool, features: Option<&str>) -> Result<()> {
    require_bootable(arch)?;
    let image = build(arch, release, features)?;

    let mut command = Command::new(arch.qemu());
    command.args(arch.machine_args());
    // -nographic implies -display none and -serial mon:stdio, so Ctrl-A X quits.
    command.args(["-nographic", "-no-reboot"]);
    command.arg("-kernel").arg(&image);
    if debug {
        // -s: gdb stub on :1234. -S: halt at reset so gdb can attach first.
        command.args(["-s", "-S"]);
        println!("xtask: QEMU halted at reset, gdb stub on localhost:1234");
    }

    println!("xtask: {} (Ctrl-A X to quit)", arch.qemu());
    let status = command
        .status()
        .map_err(|error| missing_qemu(arch, &error))?;
    if !status.success() {
        return Err(format!("{} exited with {status}", arch.qemu()).into());
    }
    Ok(())
}

/// Boots the kernel and requires `expect` to appear on the serial console.
fn boot_test(
    arch: Arch,
    release: bool,
    expect: &str,
    timeout: Duration,
    features: Option<&str>,
) -> Result<()> {
    require_bootable(arch)?;
    let image = build(arch, release, features)?;

    let mut command = Command::new(arch.qemu());
    command.args(arch.machine_args());
    // An explicit chardev rather than -nographic: the console must arrive on a
    // pipe we can read, and `signal=off` stops QEMU claiming Ctrl-C.
    command.args([
        "-display",
        "none",
        "-monitor",
        "none",
        "-chardev",
        "stdio,id=char0,signal=off",
        "-serial",
        "chardev:char0",
        "-no-reboot",
    ]);
    command.arg("-kernel").arg(&image);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    println!(
        "xtask: booting under {}, expecting {expect:?} within {}s",
        arch.qemu(),
        timeout.as_secs()
    );

    let mut child = command
        .spawn()
        .map_err(|error| missing_qemu(arch, &error))?;
    let stdout = child.stdout.take().ok_or("QEMU stdout was not captured")?;

    // The kernel halts rather than exiting, so QEMU never terminates on its own.
    // A reader thread plus a deadline is what turns that into a bounded test.
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(text) => {
                    if sender.send(text).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let deadline = Instant::now() + timeout;
    let mut transcript = Vec::new();
    let mut found = false;

    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                println!("  qemu | {line}");
                let matched = line.contains(expect);
                transcript.push(line);
                if matched {
                    found = true;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Always reap QEMU: leaving an emulator running would wedge a CI runner.
    let _ = child.kill();
    let _ = child.wait();

    if found {
        println!("xtask: found {expect:?} — boot test passed");
        return Ok(());
    }

    if transcript.is_empty() {
        return Err(format!(
            "no console output at all within {}s. The kernel did not reach its \
             console: suspect the load address in the link script, the entry \
             symbol, or the stack.",
            timeout.as_secs()
        )
        .into());
    }

    Err(format!(
        "{expect:?} did not appear within {}s. Console produced {} line(s):\n{}",
        timeout.as_secs(),
        transcript.len(),
        transcript.join("\n")
    )
    .into())
}

/// Turns a spawn failure into advice rather than an errno.
fn missing_qemu(arch: Arch, error: &std::io::Error) -> Box<dyn Error> {
    if error.kind() == std::io::ErrorKind::NotFound {
        return format!(
            "{} not found on PATH. The devcontainer provides it; on a host, \
             install QEMU (Debian: qemu-system-arm and qemu-system-x86).",
            arch.qemu()
        )
        .into();
    }
    format!("failed to start {}: {error}", arch.qemu()).into()
}
