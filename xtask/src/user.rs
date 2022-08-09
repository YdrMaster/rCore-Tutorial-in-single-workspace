use crate::{strip_all, PROJECT, TARGET, TARGET_ARCH};
use command_ext::{Cargo, CommandExt};
use once_cell::sync::Lazy;
use std::{ffi::OsStr, fs::File, io::Write, path::PathBuf};

const PACKAGE: &str = "user_lib";
static USER: Lazy<PathBuf> = Lazy::new(|| PROJECT.join("user"));

fn build_all(release: bool, base_address: u64) -> Vec<PathBuf> {
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
    names.sort_unstable();
    names
        .into_iter()
        .map(|name| build_one(name, release, base_address))
        .collect()
}

fn build_one(name: impl AsRef<OsStr>, release: bool, base_address: u64) -> PathBuf {
    let name = name.as_ref();
    Cargo::build()
        .package(PACKAGE)
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

pub fn build_for(_ch: u8, release: bool) {
    let bins = build_all(release, 0x8040_0000u64);
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
