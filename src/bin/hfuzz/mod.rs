use anyhow::Result;

use std::fs;
use std::env;
use std::process::{self, Command};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

/// The version of `cargo-hfuzz` cli tooling.
const VERSION: &str = env!("CARGO_PKG_VERSION");
const HONGGFUZZ_TARGET: &str = "hfuzz_target";
const HONGGFUZZ_WORKSPACE: &str = "hfuzz_workspace";


use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "cargo-hfuzz", about = "Fuzz your Rust code with Google-developed Honggfuzz !")]
pub(crate) struct Opt {
    #[structopt(subcommand)]
    command: OptSub,
}

/// Shared options for multiple sub-commands.
#[derive(Debug, StructOpt)]
struct CommonOpts {
    /// only build binary but don't execute it
    #[structopt(long)]
    only_build: bool,

    /// flags given to `rustc`, for example "-Z sanitizer=address"
    #[structopt(long, env = "RUSTFLAGS")]
    rustflags: Option<String>,

    /// args given to `cargo build`
    #[structopt(long, env = "HFUZZ_BUILD_ARGS")]
    build_args: Option<String>,

    /// Pass --verbose to honggfuzz, enable various log levels.
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// path to working directory
    #[structopt(short, long, default_value = "hfuzz_workspace", env = "HFUZZ_WORKSPACE")]
    workspace: String,
}

#[derive(Debug, StructOpt)]
enum OptSub {
    /// build and run fuzzing
    Fuzz {
        #[structopt(flatten)]
        common_opts: CommonOpts,

        /// path to fuzzer's input files (aka "corpus"), relative to `$HFUZZ_WORKSPACE/{TARGET}`
        #[structopt(short, long, default_value = "input", env = "HFUZZ_INPUT")]
        input: String,

        /// which binary to fuzz
        target: String,

        /// do no build with compiler instrumentation
        #[structopt(long)]
        no_instr: bool,

        /// args to the binary, followed by an optional `--` which are interpreted by the fuzzer itself
        /// ( https://github.com/google/honggfuzz/blob/master/docs/USAGE.md )
        args: Vec<String>,
    },

    /// Debug
    Debug {
        #[structopt(flatten)]
        common_opts: CommonOpts,

        /// name or path to debugger, like `rust-gdb`, `gdb`, `/usr/bin/lldb-7`..
        #[structopt(short, long, default_value = "rust-lldb", env = "HFUZZ_DEBUGGER")]
        debugger: String,

        /// which binary target to fuzz
        target: String,

        /// path to crash file, typically like `hfuzz_workspace/[TARGET]/[..].fuzz`
        crash_file: PathBuf,

        /// args to target
        target_args: Vec<String>,
    },

    /// Clean the saved fuzzing state and all related files.
    Clean,
}

impl Opt {
    pub(crate) fn verbosity(&self) -> log::LevelFilter {
        self.command.verbosity()
    }
}

impl OptSub {
    pub(crate) fn verbosity(&self) -> log::LevelFilter {
        let verbose = match self {
            OptSub::Fuzz { common_opts, ..} => common_opts.verbose,
            OptSub::Debug { common_opts, ..} => common_opts.verbose,
            OptSub::Clean => panic!("Subcommand 'clean` cannot be verbosive!"),
        };
        match verbose {
            //_ if self.flag_quiet => log::LevelFilter::Off, TODO
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            n if n > 4 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildType {
    ReleaseInstrumented,
    ReleaseNotInstrumented,
    ProfileWithGrcov,
    Debug
}


#[inline(always)]
pub(crate) fn target_triple() -> Result<String> {
    Ok(rustc_version::version_meta()?.host)
}

pub(crate) fn find_crate_root() -> Result<PathBuf> {
    let path = env::current_dir()
        .map_err(|e| anyhow::anyhow!("Current directory is not set for process.").context(e))?;
    let mut path = path.as_path();
    while !path.join("Cargo.toml").is_file() {
        // move to parent path
        path = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Reached root without finding Cargo.toml"))?;
    }

    Ok(path.to_path_buf())
}

pub(crate) fn debugger_command(target: &str, triple: &str) -> Command {
    let debugger = env::var("HFUZZ_DEBUGGER").unwrap_or_else(|_| "rust-lldb".into());
    let honggfuzz_target = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| HONGGFUZZ_TARGET.into());

    let mut cmd = Command::new(&debugger);

    let dest = format!("{}/{}/debug/{}", &honggfuzz_target, triple, target);
    match Path::new(&debugger)
        .file_name()
        .map(|f| f.to_string_lossy().contains("lldb"))
    {
        Some(true) => {
            cmd.args(&["-o", "b rust_panic", "-o", "r", "-o", "bt", "-f", &dest, "--"]);
        }
        _ => {
            cmd.args(&["-ex", "b rust_panic", "-ex", "r", "-ex", "bt", "--args", &dest]);
        }
    };

    cmd
}

pub(crate) fn hfuzz_version() {
    println!("cargo-hfuzz {}", VERSION);
}

fn hfuzz_run<T>(mut args: T, crate_root: &Path, build_type: &BuildType) where T: std::iter::Iterator<Item=String> {
    let target = args.next().unwrap_or_else(||{
        eprintln!("please specify the name of the target like this \"cargo hfuzz run[-debug|-no-instr] TARGET [ ARGS ... ]\"");
        process::exit(1);
    });

    let honggfuzz_target = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| HONGGFUZZ_TARGET.into());
    let honggfuzz_workspace = env::var("HFUZZ_WORKSPACE").unwrap_or_else(|_| HONGGFUZZ_WORKSPACE.into());
    let honggfuzz_input = env::var("HFUZZ_INPUT").unwrap_or_else(|_| format!("{}/{}/input", honggfuzz_workspace, target));

    hfuzz_build(vec!["--bin".to_string(), target.clone()].into_iter(), crate_root, build_type);

    match *build_type {
        BuildType::Debug => {
            let crash_filename = args.next().unwrap_or_else(||{
                eprintln!("please specify the crash filename like this \"cargo hfuzz run-debug TARGET CRASH_FILENAME [ ARGS ... ]\"");
                process::exit(1);
            });

            let status = debugger_command(&target)
                .args(args)
                .env("CARGO_HONGGFUZZ_CRASH_FILENAME", crash_filename)
                .env("RUST_BACKTRACE", env::var("RUST_BACKTRACE").unwrap_or_else(|_| "1".into()))
                .status()
                .unwrap();
            if !status.success() {
                 process::exit(status.code().unwrap_or(1));
            }
        }
        _ => {
            // add some flags to sanitizers to make them work with Rust code
            let asan_options = env::var("ASAN_OPTIONS").unwrap_or_default();
            let asan_options = format!("detect_odr_violation=0:{}", asan_options);

            let tsan_options = env::var("TSAN_OPTIONS").unwrap_or_default();
            let tsan_options = format!("report_signal_unsafe=0:{}", tsan_options);

            // get user-defined args for honggfuzz
            let hfuzz_run_args = env::var("HFUZZ_RUN_ARGS").unwrap_or_default();
            // FIXME: we split by whitespace without respecting escaping or quotes
            let hfuzz_run_args = hfuzz_run_args.split_whitespace();

            fs::create_dir_all(&format!("{}/{}/input", &honggfuzz_workspace, target)).unwrap_or_else(|_| {
                println!("error: failed to create \"{}/{}/input\"", &honggfuzz_workspace, target);
            });

            let command = format!("{}/honggfuzz", &honggfuzz_target);
            Command::new(&command) // exec honggfuzz replacing current process
                .args(&["-W", &format!("{}/{}", &honggfuzz_workspace, target), "-f", &honggfuzz_input, "-P"])
                .args(hfuzz_run_args) // allows user-specified arguments to be given to honggfuzz
                .args(&["--", &format!("{}/{}/release/{}", &honggfuzz_target, target_triple(), target)])
                .args(args)
                .env("ASAN_OPTIONS", asan_options)
                .env("TSAN_OPTIONS", tsan_options)
                .exec();

            // code flow will only reach here if honggfuzz failed to execute
            eprintln!("cannot execute {}, try to execute \"cargo hfuzz build\" from fuzzed project directory", &command);
            process::exit(1);
        }
    }
}

fn hfuzz_build<T>(args: T, crate_root: &Path, build_type: &BuildType) where T: std::iter::Iterator<Item=String> {
    let honggfuzz_target = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| HONGGFUZZ_TARGET.into());

    // HACK: temporary fix, see https://github.com/rust-lang/rust/issues/53945#issuecomment-426824324
    let use_gold_linker: bool = match Command::new("which") // check if the gold linker is available
            .args(&["ld.gold"])
            .status() {
        Err(_) => false,
        Ok(status) => status.code().map(|code| code == 0).unwrap_or_default(),
    };

    let mut rustflags = "\
    --cfg fuzzing \
    -C debug-assertions \
    -C overflow_checks \
    ".to_string();

    let mut cargo_incremental = "1";
    match *build_type {
        BuildType::Debug => {
            rustflags.push_str("\
            --cfg fuzzing_debug \
            -C opt-level=0 \
            -C debuginfo=2 \
            ");
        }

        BuildType::ProfileWithGrcov => {
            rustflags.push_str("\
            --cfg fuzzing_debug \
            -Zprofile \
            -Cpanic=abort \
            -C opt-level=0 \
            -C debuginfo=2 \
            -Ccodegen-units=1 \
            -Cinline-threshold=0 \
            -Clink-dead-code \
            ");
            //-Coverflow-checks=off \
            cargo_incremental = "0";
        }

        _ => {
            rustflags.push_str("\
            -C opt-level=3 \
            -C target-cpu=native \
            -C debuginfo=0 \
            ");

            if *build_type == BuildType::ReleaseInstrumented {
                rustflags.push_str("\
                -C passes=sancov \
                -C llvm-args=-sanitizer-coverage-level=4 \
                -C llvm-args=-sanitizer-coverage-trace-pc-guard \
                -C llvm-args=-sanitizer-coverage-trace-divs \
                ");

                // trace-compares doesn't work on macOS without a sanitizer
                if cfg!(not(target_os="macos")) {
                    rustflags.push_str("\
                    -C llvm-args=-sanitizer-coverage-trace-compares \
                    ");
                }

                // HACK: temporary fix, see https://github.com/rust-lang/rust/issues/53945#issuecomment-426824324
                if use_gold_linker {
                    rustflags.push_str("-Clink-arg=-fuse-ld=gold ");
                }
            }
        }
    }

    // add user provided flags
    rustflags.push_str(&env::var("RUSTFLAGS").unwrap_or_default());

    // get user-defined args for building
    let hfuzz_build_args = env::var("HFUZZ_BUILD_ARGS").unwrap_or_default();
    // FIXME: we split by whitespace without respecting escaping or quotes
    let hfuzz_build_args = hfuzz_build_args.split_whitespace();

    let cargo_bin = env::var("CARGO").unwrap();
    let mut command = Command::new(cargo_bin);
    command.args(&["build", "--target", &target_triple()]) // HACK to avoid building build scripts with rustflags
        .args(args)
        .args(hfuzz_build_args) // allows user-specified arguments to be given to cargo build
        .env("RUSTFLAGS", rustflags)
        .env("CARGO_INCREMENTAL", cargo_incremental)
        .env("CARGO_TARGET_DIR", &honggfuzz_target) // change target_dir to not clash with regular builds
        .env("CRATE_ROOT", &crate_root);

    if *build_type == BuildType::ProfileWithGrcov {
        command.env("CARGO_HONGGFUZZ_BUILD_VERSION", VERSION)   // used by build.rs to check that versions are in sync
            .env("CARGO_HONGGFUZZ_TARGET_DIR", &honggfuzz_target); // env variable to be read by build.rs script
    }                                                              // to place honggfuzz executable at a known location
    else if *build_type != BuildType::Debug {
        command.arg("--release")
            .env("CARGO_HONGGFUZZ_BUILD_VERSION", VERSION)   // used by build.rs to check that versions are in sync
            .env("CARGO_HONGGFUZZ_TARGET_DIR", &honggfuzz_target); // env variable to be read by build.rs script
    }                                                              // to place honggfuzz executable at a known location

    let status = command.status().unwrap();
    if !status.success() {
         process::exit(status.code().unwrap_or(1));
    }
}

fn hfuzz_clean<T>(args: T) where T: std::iter::Iterator<Item=String> {
    let honggfuzz_target = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| HONGGFUZZ_TARGET.into());
    let cargo_bin = env::var("CARGO").unwrap();
    let status = Command::new(cargo_bin)
        .args(&["clean"])
        .args(args)
        .env("CARGO_TARGET_DIR", &honggfuzz_target) // change target_dir to not clash with regular builds
        .status()
        .unwrap();
    if !status.success() {
         process::exit(status.code().unwrap_or(1));
    }
}

pub fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    match opt.command {
        OptSub::Fuzz { common_opts, input, target, no_instr, args } => {
            todo!("fuzz");
        },
        OptSub::Debug { common_opts, debugger, target, crash_file, target_args } => {
            todo!("debug");
        },
        OptSub::Clean => {
            todo!("clean");
        },
    }
    return;

    let mut args = env::args().skip(1);
    if args.next() != Some("hfuzz".to_string()) {
        eprintln!("please launch as a cargo subcommand: \"cargo hfuzz ...\"");
        process::exit(1);
    }

    // change to crate root to have the same behavior as cargo build/run
    let crate_root = find_crate_root().unwrap_or_else(|_| {
        eprintln!("error: could not find `Cargo.toml` in current directory or any parent directory");
        process::exit(1);
    });
    env::set_current_dir(&crate_root).unwrap();

    match args.next() {
        Some(ref s) if s == "build" => {
            hfuzz_build(args, &crate_root, &BuildType::ReleaseInstrumented);
        }
        Some(ref s) if s == "build-no-instr" => {
            hfuzz_build(args, &crate_root, &BuildType::ReleaseNotInstrumented);
        }
        Some(ref s) if s == "build-debug" => {
            hfuzz_build(args, &crate_root, &BuildType::Debug);
        }
        Some(ref s) if s == "build-grcov" => {
            hfuzz_build(args, &crate_root, &BuildType::ProfileWithGrcov);
        }
        Some(ref s) if s == "run" => {
            hfuzz_run(args, &crate_root, &BuildType::ReleaseInstrumented);
        }
        Some(ref s) if s == "run-no-instr" => {
            hfuzz_run(args, &crate_root, &BuildType::ReleaseNotInstrumented);
        }

        Some(ref s) if s == "run-debug" => {
            hfuzz_run(args, &crate_root, &BuildType::Debug);
        }
        Some(ref s) if s == "clean" => {
            hfuzz_clean(args);
        }
        Some(ref s) if s == "version" => {
            hfuzz_version();
        }
        _ => {
            eprintln!("possible commands are: run, run-no-instr, run-debug, build, build-no-instr, build-grcov, build-debug, clean, version");
            process::exit(1);
        }
    }
}
