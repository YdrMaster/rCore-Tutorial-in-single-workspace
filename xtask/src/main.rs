#[macro_use]
extern crate clap;

use clap::Parser;
use command_ext::{BinUtil, Cargo, CommandExt, Qemu};
use std::path::{Path, PathBuf};

const TARGET: &str = "riscv64imac-unknown-none-elf";

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
        Make(args) => args.make(),
        Qemu(args) => args.run(),
    }
}

#[derive(Args, Default)]
struct BuildArgs {
    /// With supervisor.
    #[clap(short, long)]
    ch: u8,
    /// Build in debug mode.
    #[clap(long)]
    debug: bool,
}

impl BuildArgs {
    /// Returns the dir of target files.
    fn dir(&self) -> PathBuf {
        Path::new(std::env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join(TARGET)
            .join(if self.debug { "debug" } else { "release" })
    }

    fn make(&self) {
        let package = format!("ch{}", self.ch);
        // 生成
        Cargo::build()
            .package(&package)
            .conditional(!self.debug, |sbi| {
                sbi.release();
            })
            .target(TARGET)
            .invoke();
        // 裁剪
        let target = self.dir().join(package);
        BinUtil::objcopy()
            .arg("--binary-architecture=riscv64")
            .arg(&target)
            .arg("--strip-all")
            .arg("-O")
            .arg("binary")
            .arg(target.with_extension("bin"))
            .invoke();
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
        self.build.make();
        if let Some(p) = &self.qemu_dir {
            Qemu::search_at(p);
        }
        Qemu::system("riscv64")
            .args(&["-machine", "virt"])
            .args(&["-bios", "default"])
            .arg("-kernel")
            .arg(self.build.dir().join("ch1.bin"))
            .args(&["-smp", &self.smp.unwrap_or(8).to_string()])
            .args(&["-serial", "mon:stdio"])
            .arg("-nographic")
            .optional(&self.gdb, |qemu, gdb| {
                qemu.args(&["-gdb", &format!("tcp::{gdb}")]);
            })
            .invoke();
    }
}
