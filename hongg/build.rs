use std::env;
use std::path::PathBuf;
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_family = "windows")]
compile_error!("honggfuzz-rs does not currently support Windows but works well under WSL (Windows Subsystem for Linux)");


#[track_caller]
fn run_cmd(cmd: &mut Command) {
    let full = Vec::from_iter(std::iter::once(cmd.get_program()).chain(cmd.get_args()).map(|x| x.to_string_lossy().to_string())).join(" ");
    let status = 
        cmd.status()
        .expect(format!("Failed to spawn process \"{full}\"").as_str());

    assert!(
        status.success(),
        "Command failed ({:?}): \"{}\"",
        &status,
        &full
    );
}

fn main() {
    let mut make = make_cmd::gnu_make();
    let cwd = dbg!(std::env::current_dir().unwrap());
    
    // Only build honggfuzz binaries if we are in the process of building an instrumentized binary
    let honggfuzz_target = match env::var("CARGO_HONGGFUZZ_TARGET_DIR") {
        Ok(path) => PathBuf::from(path), // path where to place honggfuzz binary. provided by cargo-hfuzz command.
        Err(_) => return,
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap()); // from cargo
    let crate_root = PathBuf::from(env::var("CRATE_ROOT").unwrap()); //from honggfuzz

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
        eprintln!("The version of the honggfuzz library dependency ({0}) and the version of the `cargo-hfuzz` executable ({1}) do not match.\n\
                   If updating both by running `cargo update` and `cargo install honggfuzz` does not work, you can either:\n\
                   - change the dependency in `Cargo.toml` to `honggfuzz = \"={1}\"`\n\
                   - or run `cargo install honggfuzz --version {0}`",
                  VERSION, honggfuzz_build_version);
        std::process::exit(1);
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = manifest_dir.as_str();
    
    // clean upsteam honggfuzz directory
    run_cmd(make.args(format!("-C {manifest_dir}/honggfuzz clean").split_ascii_whitespace()).current_dir(manifest_dir));
    // TODO: maybe it's not a good idea to always clean the sources..

    // build honggfuzz command and hfuzz static library
    run_cmd(make.args(format!("-C {manifest_dir}/honggfuzz honggfuzz libhfuzz/libhfuzz.a libhfcommon/libhfcommon.a").split_ascii_whitespace()).current_dir(cwd.parent().unwrap()));

    use fs_err as fs;
    
    fs::copy(format!("{manifest_dir}/honggfuzz/libhfuzz/libhfuzz.a"), out_dir.join("libhfuzz.a")).unwrap();
    fs::copy(format!("{manifest_dir}/honggfuzz/libhfcommon/libhfcommon.a"), out_dir.join("libhfcommon.a")).unwrap();
    
    // copy honggfuzz executable to honggfuzz target directory
    fs::copy(format!("{manifest_dir}/honggfuzz/honggfuzz"), honggfuzz_target.join("honggfuzz")).unwrap();

    // tell cargo how to link final executable to hfuzz static library
    println!("cargo:rustc-link-lib=static={}", "hfuzz");
    println!("cargo:rustc-link-lib=static={}", "hfcommon");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
}
