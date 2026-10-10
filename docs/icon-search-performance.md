# Icon search Windows measurements

Measured on 2026-10-10 on Windows 10 Enterprise 10.0.19045, AMD Ryzen 9
3900X, NVIDIA GeForce RTX 3070, primary monitor 3440 × 1440. OpenCV 4.12.0
MSVC Release DLLs and Rust release profile were used. These are actual DXGI
readbacks and OpenCV searches on the current desktop, not synthetic timings.

Run from the repository root:

```powershell
cargo test --release --manifest-path src-tauri/Cargo.toml --lib measures_visual_poll_on_windows -- --ignored --nocapture
```

| Workload (100 samples) | p95 |
| --- | --- |
| Pixels only, two screen corners, existing compact readback | 0.735 ms |
| Actual pixels-only start-to-start interval, 50 ms schedule | 50.619 ms |
| Two pixels + three 1200 × 800 ROIs: readback and RGB conversion | 10.24 ms |
| Three color template searches, each 24 × 24 | 227.82 ms |
| Combined ROI readback and matching | 236.82 ms |
| Actual start-to-start visual polling interval, 50 ms schedule | 236.82 ms |

The large workload exceeded the 50 ms deadline on all 100 samples. The
schedule therefore does not guarantee 50 ms completion for arbitrary search
areas. A smaller search rectangle reduces work. The benchmark records deadline
misses explicitly; results are not represented as meeting the target.

All points and ROIs come from the same acquired texture. The Windows benchmark
asserts each ROI's first pixel equals the point sampled at the same coordinate.
The pixels-only batch delegates to the original compact points path. Screen
access loss invalidates the previously acquired texture.

Polling and matching execute serially on the poll thread; there are no parallel
matching jobs or deferred frame results. The deadline scheduler skips missed
ticks rather than catching up. Configuration and action-completion commands
are drained even after an overrun so a heavy search cannot indefinitely starve
the next update. The state-machine tests cover same-frame AND, unavailable
results, next-poll changes, pause, delay, queue and cooldown.

The initial debug measurement was 998.37 ms combined p95 (446.56 ms readback,
559.34 ms matching). The final implementation borrows RGB image storage for
OpenCV input instead of constructing pixel vectors; result validity checking
and minimum reduction run in native OpenCV. Release numbers above are the
delivery performance evidence, while debug overhead remains unsuitable for
judging the 50 ms target.

Use `--lib` for backend tests: the application's binary test target inherits
the Windows elevation manifest and cannot be launched by Cargo in this
unelevated environment (Windows error 740). The library benchmark itself passed.
