# Dependencies

- Rust (`rustup`)
- Android NDK 21.0.6113669

# Setup

- copy/link/merge cargo-config.toml into ~/.cargo/config, adjusting paths as necessary
- `$ rustup target add aarch64-linux-android armv7-linux-androideabi`
- `$ ./build.sh` to build Rust project and create libraries for Android project
- Run `$ ./gradlew installRunDebug` in `android/` to build and run APK

## Troubleshooting
- `libc++_shared.so` errors: Adjust `android/app/src/main/jniLibs/*/libc++_shared.so` links
- Rust build errors: Check NDK paths in `~/.cargo/config` and `build.sh`
