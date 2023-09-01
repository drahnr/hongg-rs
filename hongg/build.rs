use std::env;
use std::path::PathBuf;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_family = "windows")]
compile_error!("honggfuzz-rs does not currently support Windows but works well under WSL (Windows Subsystem for Linux)");

#[track_caller]
fn run_cmd(cmd: &mut Command) -> anyhow::Result<()> {
    let full = Vec::from_iter(
        std::iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(|x| x.to_string_lossy().to_string()),
    )
    .join(" ");
    let status = cmd
        .status()
        .expect(format!("Failed to spawn process \"{full}\"").as_str());

    if !status.success() {
        anyhow::bail!("Command failed ({:?}): \"{}\"", &status, &full);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let mut make = make_cmd::gnu_make();

    // Only build honggfuzz binaries if we are in the process of building an instrumentized binary
    let honggfuzz_target = match env::var("CARGO_HONGGFUZZ_TARGET_DIR") {
        Ok(path) => PathBuf::from(path), // path where to place honggfuzz binary. provided by cargo-hfuzz command.
        Err(_) => return Ok(()),
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR")?); // from cargo
    let crate_root = PathBuf::from(env::var("CRATE_ROOT")?); //from honggfuzz

    let honggfuzz_target = if honggfuzz_target.is_absolute() {
        // in case CARGO_HONGGFUZZ_TARGET_DIR was initialized
        // from an absolute CARGO_TARGET_DIR we should not
        // prepend the crate root again
        honggfuzz_target
    } else {
        crate_root.join(honggfuzz_target)
    };

    // check that "cargo hongg" command is at the same version as this file
    let honggfuzz_build_version =
        env::var("CARGO_HONGGFUZZ_BUILD_VERSION").unwrap_or("unknown".to_string());
    if VERSION != honggfuzz_build_version {
        anyhow::bail!("The version of the honggfuzz library dependency ({0}) and the version of the `cargo-hfuzz` executable ({1}) do not match.\n\
                   If updating both by running `cargo update` and `cargo install honggfuzz` does not work, you can either:\n\
                   - change the dependency in `Cargo.toml` to `honggfuzz = \"={1}\"`\n\
                   - or run `cargo install honggfuzz --version {0}`",
                  VERSION, honggfuzz_build_version);
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let manifest_dir = manifest_dir.canonicalize()?;
    let manifest_dir = manifest_dir.as_path();

    // clean upsteam honggfuzz directory
    run_cmd(
        make.args("-C honggfuzz clean".split_ascii_whitespace())
            .current_dir(manifest_dir),
    )?;
    // TODO: maybe it's not a good idea to always clean the sources..

    // build honggfuzz command and hfuzz static library
    run_cmd(
        make.args(
            "-C honggfuzz honggfuzz libhfuzz/libhfuzz.a libhfcommon/libhfcommon.a"
                .split_ascii_whitespace(),
        )
        .current_dir(manifest_dir),
    )?;

    use fs_err as fs;

    fs::copy("honggfuzz/libhfuzz/libhfuzz.a", out_dir.join("libhfuzz.a"))?;
    fs::copy(
        "honggfuzz/libhfcommon/libhfcommon.a",
        out_dir.join("libhfcommon.a"),
    )?;

    // copy honggfuzz executable to honggfuzz target directory
    fs::copy("honggfuzz/honggfuzz", honggfuzz_target.join("honggfuzz"))?;

    // tell cargo how to link final executable to hfuzz static library
    println!("cargo:rustc-link-lib=static={}", "hfuzz");
    println!("cargo:rustc-link-lib=static={}", "hfcommon");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    Ok(())
}
