//! 把图标与版本信息编进 exe（资源段），这样资源管理器 / 任务栏 /
//! 「属性 → 详细信息」里都有正经的名字和图标，而不是一个无名 exe。
//!
//! 版本号只有 `Cargo.toml` 一处是真的：模板 `assets/app.rc` 里放占位符，
//! 这里替换成 `CARGO_PKG_VERSION` 再交给 windres，避免两处数字对不上。
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
    let Ok(out_dir) = std::env::var("OUT_DIR") else {
        return;
    };
    let ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());

    // VERSIONINFO 要的是 4 段数字：0.1.0 → 0,1,0,0
    let tuple = {
        let mut parts: Vec<String> = ver
            .split('.')
            .map(|s| s.chars().take_while(char::is_ascii_digit).collect())
            .collect();
        while parts.len() < 4 {
            parts.push("0".into());
        }
        parts.truncate(4);
        parts.join(",")
    };

    let Ok(tpl) = std::fs::read_to_string("assets/app.rc") else {
        println!("cargo:warning=读不到 assets/app.rc，跳过图标资源");
        return;
    };
    let rc = tpl.replace("__VER_TUPLE__", &tuple).replace("__VER__", &ver);
    let rc_path = format!("{out_dir}/app-res.rc");
    if std::fs::write(&rc_path, rc).is_err() {
        println!("cargo:warning=生成 app-res.rc 失败，跳过图标资源");
        return;
    }

    let obj = format!("{out_dir}/app-res.o");
    let ok = Command::new("windres")
        .args([
            "--codepage=65001",
            "-I",
            "assets",
            "--input",
            &rc_path,
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
