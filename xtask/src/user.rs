use crate::{fs_pack::easy_fs_pack, objcopy, PROJECT, TARGET, TARGET_ARCH};
use os_xtask_utils::{Cargo, CommandExt};
use serde_derive::Deserialize;
use std::{collections::HashMap, ffi::OsStr, fs::File, io::Write, path::PathBuf};

#[derive(Deserialize, Default)]
struct Cases {
    base: Option<u64>,
    step: Option<u64>,
    pub cases: Option<Vec<String>>,
}

pub struct CasesInfo {
    base: u64,
    step: u64,
    bins: Vec<PathBuf>,
}

impl Cases {
    fn build(&mut self, release: bool) -> CasesInfo {
        if let Some(names) = &self.cases {
            let base = self.base.unwrap_or(0);
            let step = self.step.filter(|_| self.base.is_some()).unwrap_or(0);
            let cases = names
                .into_iter()
                .enumerate()
                .map(|(i, name)| build_one(name, release, base + i as u64 * step))
                .collect();
            CasesInfo {
                base,
                step,
                bins: cases,
            }
        } else {
            CasesInfo {
                base: 0,
                step: 0,
                bins: vec![],
            }
        }
    }
}

fn build_one(name: impl AsRef<OsStr>, release: bool, base_address: u64) -> PathBuf {
    let name = name.as_ref();
    let binary = base_address != 0;
    if binary {
        println!("build {name:?} at {base_address:#x}");
    }
    Cargo::build()
        .package("user_lib")
        .target(TARGET_ARCH)
        .arg("--bin")
        .arg(name)
        .conditional(release, |cargo| {
            cargo.release();
        })
        .conditional(binary, |cargo| {
            cargo.env("BASE_ADDRESS", base_address.to_string());
        })
        .invoke();
    let elf = TARGET
        .join(if release { "release" } else { "debug" })
        .join(name);
    if binary {
        objcopy(elf, binary)
    } else {
        elf
    }
}

pub fn build_for(ch: u8, release: bool) {
    let cfg = std::fs::read_to_string(PROJECT.join("user/cases.toml")).unwrap();
    let mut cases = toml::from_str::<HashMap<String, Cases>>(&cfg)
        .unwrap()
        .remove(&format!("ch{ch}"))
        .unwrap_or_default();
    let CasesInfo { base, step, bins } = cases.build(release);
    if bins.is_empty() {
        return;
    }
    let asm = TARGET
        .join(if release { "release" } else { "debug" })
        .join("app.asm");
    let mut ld = File::create(asm).unwrap();
    writeln!(
        ld,
        "\
    .global apps
    .section .data
    .align 3
apps:
    .quad {base:#x}
    .quad {step:#x}
    .quad {}",
        bins.len(),
    )
    .unwrap();

    (0..bins.len()).for_each(|i| writeln!(ld, "    .quad app_{i}_start").unwrap());

    writeln!(ld, "    .quad app_{}_end", bins.len() - 1).unwrap();

    bins.iter().enumerate().for_each(|(i, path)| {
        writeln!(
            ld,
            "
app_{i}_start:
    .incbin {path:?}
app_{i}_end:",
        )
        .unwrap();
    });

    if ch == 5 {
        writeln!(
            ld,
            "
    .align 3
    .section .data
    .global app_names
app_names:"
        )
        .unwrap();
        bins.iter().enumerate().for_each(|(_, path)| {
            writeln!(ld, "    .string {:?}", path.file_name().unwrap()).unwrap();
        });
    } else if ch >= 6 {
        easy_fs_pack(
            &cases.cases.unwrap(),
            TARGET
                .join(if release { "release" } else { "debug" })
                .into_os_string()
                .into_string()
                .unwrap()
                .as_str(),
        )
        .unwrap();
    }
}
