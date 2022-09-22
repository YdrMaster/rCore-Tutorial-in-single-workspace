fn main() {
    use std::{env, fs, path::PathBuf};

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LOG");
    println!("cargo:rerun-if-env-changed=BASE_ADDRESS");

    if let Some(base) = env::var("BASE_ADDRESS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
    {
        let ld = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
        let text = format!("BASE_ADDRESS = {base:#x};{LINKER}",);
        fs::write(ld, text).unwrap();
        println!("cargo:rustc-link-arg=-T{}", ld.display());
    }
}

const LINKER: &str = "
OUTPUT_ARCH(riscv)
ENTRY(_start)
SECTIONS {
    . = BASE_ADDRESS;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        *(.bss.uninit)
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
}";
