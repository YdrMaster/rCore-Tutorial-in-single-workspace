mod fs_pack;
mod user;

#[macro_use]
extern crate clap;

use clap::Parser;
use once_cell::sync::Lazy;
use os_xtask_utils::{BinUtil, Cargo, CommandExt, Qemu};
use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

const TARGET_ARCH: &str = "riscv64gc-unknown-none-elf";

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
    Asm(AsmArgs),
    Qemu(QemuArgs),
}

fn main() {
    use Commands::*;
    match Cli::parse().command {
        Make(args) => {
            let _ = args.make();
        }
        Asm(args) => args.dump(),
        Qemu(args) => args.run(),
    }
}

#[derive(Args, Default)]
struct BuildArgs {
    /// chapter number
    #[clap(short, long)]
    ch: u8,
    /// lab or not
    #[clap(long)]
    lab: bool,
    /// features
    #[clap(short, long)]
    features: Option<String>,
    /// log level
    #[clap(long)]
    log: Option<String>,
    /// build in release mode
    #[clap(long)]
    release: bool,
}

impl BuildArgs {
    fn make(&self) -> PathBuf {
        let mut env: HashMap<&str, OsString> = HashMap::new();
        let package = match self.ch {
            1 => if self.lab { "ch1-lab" } else { "ch1" }.to_string(),
            2..=8 => {
                user::build_for(self.ch, false);
                env.insert(
                    "APP_ASM",
                    TARGET
                        .join("debug")
                        .join("app.asm")
                        .as_os_str()
                        .to_os_string(),
                );
                format!("ch{}", self.ch)
            }
            _ => unreachable!(),
        };
        // 生成
        let mut build = Cargo::build();
        build
            .package(&package)
            .optional(&self.features, |cargo, features| {
                cargo.features(false, features.split_whitespace());
            })
            .optional(&self.log, |cargo, log| {
                cargo.env("LOG", log);
            })
            .conditional(self.release, |cargo| {
                cargo.release();
            })
            .target(TARGET_ARCH);
        for (key, value) in env {
            build.env(key, value);
        }
        build.invoke();
        TARGET
            .join(if self.release { "release" } else { "debug" })
            .join(package)
    }
}

#[derive(Args)]
struct AsmArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Output file.
    #[clap(short, long)]
    console: Option<String>,
}

impl AsmArgs {
    fn dump(self) {
        let elf = self.build.make();
        let out = Path::new(std::env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join(self.console.unwrap_or(format!("ch{}.asm", self.build.ch)));
        println!("Asm file dumps to '{}'.", out.display());
        fs::write(out, BinUtil::objdump().arg(elf).arg("-d").output().stdout).unwrap();
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
        let elf = self.build.make();
        if let Some(p) = &self.qemu_dir {
            Qemu::search_at(p);
        }
        let mut qemu = Qemu::system("riscv64");
        qemu.args(&["-machine", "virt"])
            .arg("-nographic")
            .arg("-bios")
            .arg(PROJECT.join("rustsbi-qemu.bin"))
            .arg("-kernel")
            .arg(objcopy(elf, true))
            .args(&["-smp", &self.smp.unwrap_or(1).to_string()])
            .args(&["-m", "64M"])
            .args(&["-serial", "mon:stdio"]);
        if self.build.ch > 5 {
            // Add VirtIO Device
            qemu.args(&[
                "-drive",
                format!(
                    "file={},if=none,format=raw,id=x0",
                    TARGET
                        .join(if self.build.release {
                            "release"
                        } else {
                            "debug"
                        })
                        .join("fs.img")
                        .into_os_string()
                        .into_string()
                        .unwrap()
                )
                .as_str(),
            ])
            .args(&[
                "-device",
                "virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0",
            ]);
        }
        qemu.optional(&self.gdb, |qemu, gdb| {
            qemu.args(&["-S", "-gdb", &format!("tcp::{gdb}")]);
        })
        .invoke();
    }
}

fn objcopy(elf: impl AsRef<Path>, binary: bool) -> PathBuf {
    let elf = elf.as_ref();
    let bin = elf.with_extension("bin");
    BinUtil::objcopy()
        .arg(elf)
        .arg("--strip-all")
        .conditional(binary, |binutil| {
            binutil.args(["-O", "binary"]);
        })
        .arg(&bin)
        .invoke();
    bin
}
