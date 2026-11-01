#!/usr/bin/env bash
# 一条命令出安装包。版本号只在 Cargo.toml 写一次，其余地方都从这里读：
#   - exe 资源段（build.rs 读 CARGO_PKG_VERSION）
#   - 设置页「关于」里的版本（env!("CARGO_PKG_VERSION")）
#   - 安装包文件名与注册表 DisplayVersion（-DAPP_VER 传进 NSIS）
set -euo pipefail
cd "$(dirname "$0")/.."

VER=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
if [ -z "$VER" ]; then
    echo "读不到 Cargo.toml 里的版本号" >&2
    exit 1
fi
echo "== 小舟沙盒 $VER =="

# 程序在跑就占着 exe，链接会失败
taskkill //IM xiaozhou-sandbox.exe //F >/dev/null 2>&1 || true

cargo build --release

OUT="dist/小舟沙盒-$VER-安装包.exe"
rm -f "$OUT"
MSYS_NO_PATHCONV=1 makensis -DAPP_VER="$VER" installer/setup.nsi

if [ ! -f "$OUT" ]; then
    echo "打包失败：没有生成 $OUT" >&2
    exit 1
fi

SIZE=$(stat -c %s "$OUT")
HASH=$(sha256sum "$OUT" | cut -d' ' -f1)
echo
echo "产物   $OUT"
echo "大小   $SIZE 字节"
echo "SHA256 $HASH"
