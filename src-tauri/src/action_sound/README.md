# Action sound catalog

`action_sound::catalog()` is the single ordered source of the 20 stable ids and
English names. Ids are persisted in action settings and must remain stable.
`None` represents silence; it is not a synthesized catalog entry.

All cues are deterministic, offline, 44,100 Hz mono signed 16-bit PCM. The source
uses sine partials, linear frequency sweeps, and seeded filtered noise. Bell and
gong partials are inharmonic; marimba and pluck partials are harmonic. Clicks,
echoes, paired tones, and three beeps use separate temporal events. Every event
uses cosine attack/release ramps and exponential decay. Final normalization
targets RMS 0.17 and caps peak amplitude at 0.75, allowing per-signal gain without
clipping. The base PCM is lazily cached once per sound. Playback scales a private
copy by `volumePercent / 100`; it never changes mixer or system volume.

The worker has one pending preview (new values supersede previous pending
requests), one active preview, at most four pending runtime signals, and at most
four active runtime voices. Oldest runtime requests and voices are evicted at
their respective caps. Runtime submission performs only mailbox insertion; the
worker owns synthesis and device calls. Preview completion reports device errors
to the request; runtime errors are logged and do not stop the worker.

Each native voice has its own waveOut handle and owns a boxed, stable header and
PCM allocation. Replacement and shutdown call reset, unprepare, then close;
the memory is freed only after successful unprepare. A driver returning
`WAVERR_STILLPLAYING` is retried after reset, bounded to 20 one-millisecond waits
so a stuck driver cannot block preview or app shutdown forever. On an exceptional driver failure
that cannot unprepare, driver-owned allocations are retained to prevent use
after free. Explicit app-exit `shutdown()` joins the worker even if executor
clones remain alive; dropping the final service owner also joins it.

Export a temporary review set with:

```
cd src-tauri
cargo test export_action_sound_wavs -- --ignored
```

The 20 files appear in ignored `src-tauri/target/action-sound-review`. The export
uses 100% signal volume. Automated tests verify sample shape, amplitude bounds,
duration (80–700 ms), zero volume, caching, distinct PCM, bounded mailboxes,
preview replacement, errors, voice release, and native cleanup ordering. They do
not establish perceived loudness or auditory distinctness: listen to every file
on the target device before marking the manual review complete.
