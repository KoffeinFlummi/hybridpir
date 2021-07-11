#!/bin/bash

cd rust

# Needed for compiling C++ in sealpir-rust and submodules
export PATH=$PATH:/opt/android-sdk/ndk/21.0.6113669/toolchains/llvm/prebuilt/linux-x86_64/bin

sed -i 's/^#crate-type/crate-type/' Cargo.toml

rustup run nightly cargo build --target aarch64-linux-android --release
rustup run nightly cargo build --target armv7-linux-androideabi --release

sed -i 's/^crate-type/#crate-type/' Cargo.toml
