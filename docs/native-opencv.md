# Windows OpenCV toolchain

Requires x64 Windows, Visual Studio C++ desktop development (MSVC and Windows SDK),
Rust MSVC, CMake, Ninja and 7-Zip (`7z` in PATH). Run from the repository root:

```powershell
./scripts/prepare-opencv.ps1
cargo check --manifest-path src-tauri/Cargo.toml
cargo run --manifest-path src-tauri/Cargo.toml --example opencv_smoke
npm run tauri build
```

The preparation script downloads checksum-pinned LLVM 21.1.8 and OpenCV 4.12.0.
LLVM is extracted locally under the ignored `src-tauri/native` directory. It does
not replace a system LLVM installation. Rust bindings use `opencv = 0.101.1` with
only `clang-runtime` and `imgproc` enabled. Native OpenCV builds shared `core` and
`imgproc` with MSVC in Release mode. LLVM 13 cannot parse the current MSVC headers.

The script finds the newest **numeric version** under Visual Studio's
`VC/Redist/MSVC`, then its x64 CRT directory. The `v145` alias is not a version.
It copies release CRT DLLs and OpenCV DLLs to `native/runtime`, together with the
OpenCV license. Cargo's build script copies DLLs beside executables, tests and
examples. Tauri bundles the runtime files beside the installed application.
Users do not need OpenCV or LLVM in PATH. LLVM and OpenCV headers are build tools
and are not bundled. CI must run the same preparation script before Cargo/Tauri.

Smoke test: `opencv_smoke` matches a color template at `(1, 1)` using
`TM_SQDIFF_NORMED`, and asserts its position and finite zero error. Run the built
example directly with a PATH containing only Windows system directories to check
that the runtime DLLs resolve beside the executable.

Validated on Windows with Visual Studio 18 / MSVC 14.51.36231:
preparation completed, `cargo check` passed both from the root and `src-tauri`,
debug and release smoke tests found the color template at `(1, 1)`, and the NSIS
release installer built successfully. Its file list contains both OpenCV DLLs,
the release x64 CRT DLLs and the OpenCV license.
