use std::error::Error;

use vergen::{BuildBuilder, CargoBuilder, Emitter, RustcBuilder, SysinfoBuilder};
use vergen_git2::Git2Builder;

fn main() -> Result<(), Box<dyn Error>> {
    // If you want tighter rerun behavior, uncomment these:
    // println!("cargo:rerun-if-changed=build.rs");
    // println!("cargo:rerun-if-changed=.git/HEAD");
    // println!("cargo:rerun-if-changed=.git/index");

    let build = BuildBuilder::all_build()?;
    let cargo = CargoBuilder::all_cargo()?;
    let rustc = RustcBuilder::all_rustc()?;
    let si = SysinfoBuilder::all_sysinfo()?;
    let git = Git2Builder::all_git()?;

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&cargo)?
        .add_instructions(&rustc)?
        .add_instructions(&si)?
        .add_instructions(&git)?
        .emit()?;

    Ok(())
}
