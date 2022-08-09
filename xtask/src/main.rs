#![feature(path_file_prefix)]

mod user;

#[macro_use]
extern crate clap;

use clap::Parser;
use command_ext::{BinUtil, Cargo, CommandExt, Qemu};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

const TARGET_ARCH: &str = "riscv64imac-unknown-none-elf";

static PROJECT: Lazy<&'static Path> =
    Lazy::new(|| Path::new(std::env!("CARGO_MANIFEST_DIR")).parent().unwrap());

static TARGET: Lazy<PathBuf> = Lazy::new(|| PROJECT.join("target").join(TARGET_ARCH));

#[derive(Parser)]
#[clap(name = "rCore-Tutorial")]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Make(BuildArgs),
    Qemu(QemuArgs),
}

fn main() {
    use Commands::*;
    match Cli::parse().command {
        Make(args) => {
            let _ = args.make();
        }
        Qemu(args) => args.run(),
    }
}

#[derive(Args, Default)]
struct BuildArgs {
    /// Character.
    #[clap(short, long)]
    ch: u8,
    /// Lab?
    #[clap(long)]
    lab: bool,
    /// Build in debug mode.
    #[clap(long)]
    release: bool,
}

impl BuildArgs {
    fn make(&self) -> PathBuf {
        let package = if self.lab {
            format!("ch{}-lab", self.ch)
        } else {
            format!("ch{}", self.ch)
        };
        let mut env = HashMap::new();
        match self.ch {
            1 => {}
            2 => {
                user::build_for(2, false);
                env.insert("APP_BASE", OsString::from("0x80400000"));
                env.insert(
                    "APP_ASM",
                    TARGET
                        .join("debug")
                        .join("app.asm")
                        .as_os_str()
                        .to_os_string(),
                );
            }
            _ => unreachable!(),
        };
        // 生成
        let mut build = Cargo::build();
        build
            .package(&package)
            .conditional(self.release, |sbi| {
                sbi.release();
            })
            .target(TARGET_ARCH);
        for (key, value) in env {
            build.env(key, value);
        }
        build.invoke();
        // 裁剪
        let elf = TARGET
            .join(if self.release { "release" } else { "debug" })
            .join(package);
        strip_all(elf)
    }
}

#[derive(Args)]
struct QemuArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Path of executable qemu-system-x.
    #[clap(long)]
    qemu_dir: Option<String>,
    /// Number of hart (SMP for Symmetrical Multiple Processor).
    #[clap(long)]
    smp: Option<u8>,
    /// Port for gdb to connect. If set, qemu will block and wait gdb to connect.
    #[clap(long)]
    gdb: Option<u16>,
}

impl QemuArgs {
    fn run(self) {
        let bin = self.build.make();
        if let Some(p) = &self.qemu_dir {
            Qemu::search_at(p);
        }
        Qemu::system("riscv64")
            .args(&["-machine", "virt"])
            .arg("-bios")
            .arg(PROJECT.join("rustsbi-qemu.bin"))
            .arg("-kernel")
            .arg(bin)
            .args(&["-smp", &self.smp.unwrap_or(1).to_string()])
            .args(&["-serial", "mon:stdio"])
            .arg("-nographic")
            .optional(&self.gdb, |qemu, gdb| {
                qemu.args(&["-S", "-gdb", &format!("tcp::{gdb}")]);
            })
            .invoke();
    }
}

fn strip_all(elf: impl AsRef<Path>) -> PathBuf {
    let elf = elf.as_ref();
    let bin = elf.with_extension("bin");
    BinUtil::objcopy()
        .arg("--binary-architecture=riscv64")
        .arg(elf)
        .args(["--strip-all", "-O", "binary"])
        .arg(&bin)
        .invoke();
    bin
}
