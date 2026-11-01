//! 把图标与版本信息编进 exe（资源段），这样资源管理器 / 任务栏 /
//! 「属性 → 详细信息」里都有正经的名字和图标，而不是一个无名 exe。
//!
//! 只在 GNU 工具链下用 windres；拿不到 windres 时只警告、不阻断构建
//! （图标资源是锦上添花，编译不过才是事故）。

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = match std::env::var("OUT_DIR") {
        Ok(v) => v,
        Err(_) => return,
    };
    let obj = format!("{out_dir}/app-res.o");

    let ok = Command::new("windres")
        .args([
            "--codepage=65001",
            "--input",
            "assets/app.rc",
            "--output",
            &obj,
            "--output-format=coff",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if ok {
        println!("cargo:rustc-link-arg={obj}");
    } else {
        println!("cargo:warning=windres 不可用，跳过图标与版本资源（程序照常可用）");
    }
}
