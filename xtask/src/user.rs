use crate::*;
use command_ext::{Cargo, CommandExt};
use once_cell::sync::Lazy;
use std::{ffi::OsStr, fs::File, io::Write, path::PathBuf};

const PACKAGE: &str = "user_lib";
static USER: Lazy<PathBuf> = Lazy::new(|| PROJECT.join("user"));

/// load all applications
/// 
/// return PathBuf [ a list of location for binary mirror image ]
fn build_all(release: bool, base_addr: impl Fn(u64) -> u64) -> Vec<PathBuf> {
    // [`PathBuf` representation and layout are considered implementation detail, are not documented and must not be relied upon.]
    // get all applications names
    let mut names = USER
        .join("src/bin")
        .read_dir()
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|entry| entry.file_type().map_or(false, |t| t.is_file()))
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(OsStr::new("rs")))
        .map(|path| path.file_prefix().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    // ???
    names.sort_unstable();
    // for build each application by name(string)
    // place the application consecutively[连续]
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| build_one(name, release, base_addr(i as _)))
        // using base_address closure function to account the space where the applications should be put.
        .collect()
}

fn build_one(name: impl AsRef<OsStr>, release: bool, base_address: u64) -> PathBuf {
    // reference name
    let name = name.as_ref();
    println!("build {name:?} at {base_address:#x}");
    // Generate the ELF executable file
    Cargo::build()
        .package(PACKAGE)                       // was the same as workspace::members
        .target(TARGET_ARCH)
        .arg("--bin")
        .arg(name)
        .conditional(release, |cargo| {
            cargo.release();
        })
        .env("BASE_ADDRESS", base_address.to_string())
        .invoke();
    let elf = TARGET
        .join(if release { "release" } else { "debug" })
        .join(name);
    strip_all(elf)
}

pub fn build_for(ch: u8, release: bool) {
    // get binary mirror image
    let bins = match ch {
        // using different kinds closure function to place application in different location
        2 => build_all(release, |_| CH2_APP_BASE),
        3 => build_all(release, |i| CH3_APP_BASE + i * CH3_APP_STEP),
        _ => unreachable!(),
    };
    if let Some(first) = bins.first() {
        let mut ld = File::create(first.parent().unwrap().join("app.asm")).unwrap();
        writeln!(
            ld,
            "\
    .align 3
    .section .data
    .global _num_app
_num_app:
    .quad {}",
            bins.len(),
        )
        .unwrap();

        (0..bins.len()).for_each(|i| {
            writeln!(
                ld,
                "\
    .quad app_{i}_start"
            )
            .unwrap()
        });

        writeln!(
            ld,
            "\
    .quad app_{}_end",
            bins.len() - 1
        )
        .unwrap();

        bins.iter().enumerate().for_each(|(i, path)| {
            writeln!(
                ld,
                "
    .section .data
    .global app_{i}_start
    .global app_{i}_end
app_{i}_start:
    .incbin {path:?}
app_{i}_end:",
            )
            .unwrap();
        });
    }
}
