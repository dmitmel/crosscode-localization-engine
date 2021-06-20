use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
  println!("cargo:rerun-if-changed=build.rs");

  fn generate_nice_version() -> Option<()> {
    // <https://github.com/rust-lang/rustfmt/blob/927561ace1ef9485206a9e6a9482e39fb3e1f31b/build.rs>
    // <https://stackoverflow.com/a/44407625/12005228>
    // <https://github.com/rust-lang/rust/blob/7e717e99be6d3418d44ec510e142484db12fd757/src/bootstrap/channel.rs>
    // <https://github.com/rust-lang/rust/blob/7e717e99be6d3418d44ec510e142484db12fd757/src/bootstrap/tool.rs#L265-L274>
    // <https://github.com/rust-lang/cargo/blob/e870eac9967b132825116525476d6875c305e4d8/src/cargo/lib.rs#L181-L185>
    // <https://github.com/fusion-engineering/rust-git-version/blob/f40e32dbb0cb8cfde94e1570eac9f7f739de0655/git-version-macro/src/lib.rs#L124-L162>
    // <https://github.com/kinnison/git-testament/blob/4aaffc9aad053be50dccb4591af70a0a854f9e32/git-testament-derive/src/lib.rs>
    // <https://github.com/rustyhorde/vergen/tree/21aec3fc6624b8e04332f8eb2659ddecbbc7689d>

    let git_dir = Path::new(".git");
    // <https://git-scm.com/docs/gitrepository-layout/>
    println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
    println!("cargo:rerun-if-changed={}", git_dir.join("refs").display());

    let version = env::var("CARGO_PKG_VERSION").ok()?;

    let git_output = Command::new("git")
      .arg(format!("--git-dir={}", git_dir.display()))
      .args(&["log", "-1", "--date=short", "--pretty=format:%h %cd"])
      .output()
      .ok()?;
    if git_output.status.success() {
      let commit = String::from_utf8(git_output.stdout).ok()?;
      let nice_version = format!("{} ({})", version, commit).replace("\n", "");
      println!("cargo:rustc-env=CARGO_PKG_NICE_VERSION={}", nice_version);
    }

    Some(())
  }
  generate_nice_version();
}
