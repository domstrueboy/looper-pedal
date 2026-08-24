# Guitar Looper — Implementation Plan

A minimal, single-track looper pedal replacement for practicing guitar through
an ASIO audio interface. Standalone Windows app — no DAW, no plugin host.

## Goal

Mimic a classic single-footswitch looper pedal (e.g. TC Electronic Ditto):
one control (button + spacebar), always-on live monitoring, one loop track,
no overdub.

## Signal flow

```
Guitar --> ASIO in --> live monitor (always audible)
                    --> mixed with loop playback --> ASIO out
```

Live signal always passes through, regardless of loop state. The loop is
layered on top only once one exists and is in the Looping state.

NAM (Neural Amp Modeler) integration is explicitly OUT of scope for this
version — clean signal only. The signal path should leave room for an
insert-effect stage between input and the mix so this can be added later
without restructuring.

## Control / state machine

Single control: spacebar (and a GUI button). Mimics a real pedal:

1. **Idle** (empty) --press--> **Recording**
2. **Recording** --press--> **Looping** (recording stops, loop length is now
   fixed, playback starts immediately, loops seamlessly)
3. **Looping** --press--> **Stopped** (playback silenced, loop stays in memory)
4. **Stopped** --press--> **Looping** (resumes the same loop from the top)

**Long-press (~2s hold)**, from any state --> **Clear**: drop the loop
buffer, return to Idle. Fires the moment the hold crosses the threshold
(while still held), not on key-up — a short press only triggers its
state-cycle action on release, once it's confirmed *not* to be a long press.

## Visual feedback (MVP — no waveform yet)

| State | Indicator | Extra info |
|---|---|---|
| Idle | gray hollow circle, "Empty" | — |
| Recording | red pulsing circle, "Recording" | live elapsed time |
| Looping | green circle, "Looping" | loop-position progress bar (0-100% per cycle), loop duration |
| Stopped | amber circle, "Stopped" | loop duration still shown |

Actual waveform rendering is deferred to v2 (needed there anyway for
trim-start/end).

## Tech stack

- **Rust**
- **cpal** (ASIO backend) for audio I/O
- **egui** for the GUI (state indicator, button, ASIO device/sample-rate
  picker, keybindings)
- Real-time-safe ring buffer for the loop, pre-allocated, no allocation or
  locking inside the audio callback

## Architecture note

No heavy upfront design beyond the module sketch below — the project is
small enough to stay easy to see at a glance, and structure can evolve as
we build. The one boundary worth getting right from the start (hard to
retrofit later): the real-time audio callback stays strictly separate
from UI/state logic. No locks or allocations on the audio thread; state
changes (button presses, mode transitions) communicate to/from it via a
lock-free channel or atomics only.

No separate UI library either — egui's immediate-mode style already gives
reuse for free. UI pieces (state indicator, progress bar, button, device
picker) are small standalone functions/structs in `ui/`, e.g.
`fn state_indicator(ui: &mut egui::Ui, state: &LoopState)`, called from
wherever needed. Only worth factoring further if duplication actually
shows up later — not upfront.

## Workflow

Commit after each implementation step (see the numbered list below) with
a small, descriptive message — keeps history reviewable step by step
instead of one large diff at the end.

## Build prerequisites (Windows side)

- Rust via rustup, **MSVC toolchain** (`x86_64-pc-windows-msvc`)
- Visual Studio Build Tools — "Desktop development with C++" workload
  (needed for linking, and for compiling the ASIO SDK C++ shim)
- LLVM/libclang (needed by `bindgen`, which `asio-sys` uses to generate
  bindings to the ASIO SDK headers)
- Steinberg ASIO SDK — as of Oct 2025 it's dual-licensed (GPLv3 or
  proprietary). `asio-sys` (used by `cpal`) auto-downloads it during
  `cargo build` if `CPAL_ASIO_DIR` is left unset — nothing to fetch by
  hand. Fallback if that ever fails: grab the GPLv3 copy from
  `github.com/audiosdk/asio` (no registration needed) and point
  `CPAL_ASIO_DIR` at it.

## Project structure (proposed)

```
looper-pedal/
  Cargo.toml
  src/
    main.rs          # app entry, egui window setup
    audio/
      mod.rs
      engine.rs       # cpal stream setup, callback, state machine
      loop_buffer.rs  # ring buffer for the recorded loop
    ui/
      mod.rs
      indicator.rs    # state circle + progress bar widget
    input.rs          # spacebar/button handling, long-press detection
  PLAN.md
```

## MVP feature list

- [ ] ASIO device + sample rate selection at startup
- [ ] Live passthrough (guitar in -> out, always on)
- [ ] Single-track record / loop / stop / resume via the 4-state cycle
- [ ] Long-press (~2s) clear from any state
- [ ] Spacebar + GUI button both trigger the same control
- [ ] State + basic progress visualization per the table above

## Implementation steps

Small, ordered, each one runnable/verifiable before moving to the next.

1. **Project skeleton** — `Cargo.toml` with `cpal` (asio feature) + `egui`/`eframe`
   deps, `main.rs` opens a blank window.
   *Verify:* window opens, closes cleanly.
2. **List ASIO devices** — enumerate hosts/devices via `cpal`, print them
   (console is fine, no UI yet).
   *Verify:* "Audient USB Audio ASIO Driver" shows up in the list.
3. **Raw passthrough** — open an ASIO input+output stream, copy input
   straight to output. No loop logic at all yet.
   *Verify:* guitar audible live through headphones, latency feels okay.
4. **Loop buffer (data structure only)** — `loop_buffer.rs`: pre-allocated
   ring buffer, write/read methods. No audio wiring yet — plain unit tests.
   *Verify:* unit tests pass (write then read back matches).
5. **State machine (logic only)** — `Idle -> Recording -> Looping -> Stopped`
   + long-press Clear, as a plain struct with transition tests. Still no
   audio wiring.
   *Verify:* unit tests cover all transitions incl. long-press clear.
6. **Wire state machine into the audio callback** — recording writes into
   the loop buffer, Looping state reads/loops it and mixes with the live
   passthrough. State communicated into the callback via atomics/a
   lock-free channel (per the architecture note above).
   *Verify:* can record a loop and hear it repeat, triggered by a
   hardcoded/temporary key or timer.
7. **Spacebar + long-press input handling** — `input.rs` detects short vs
   ~2s-hold press, sends transitions to the engine.
   *Verify:* full record/loop/stop/resume/clear cycle works from the
   keyboard alone.
8. **GUI state indicator** — colored circle + label + loop-position
   progress bar, reflecting the engine's current state live.
   *Verify:* indicator updates correctly through a full cycle.
9. **GUI button** — wire the on-screen button to the same control path as
   spacebar, so either one drives the same state machine.
   *Verify:* button and spacebar are interchangeable, never desync.
10. **Startup device/sample-rate picker + basic error handling** — replace
    the hardcoded device from step 3 with a real chooser; handle
    device-open failures gracefully instead of panicking.
    *Verify:* app starts, lets you pick the Audient interface, runs end
    to end as a real MVP.
11. **Docs (initial pass)** — once the MVP works end to end:
    - *Technical* (`README.md` or `docs/technical.md`): architecture
      overview, module layout, how the audio thread/state machine talk to
      each other, build/setup steps (toolchain, ASIO SDK auto-fetch).
    - *User* (`docs/user-guide.md`): what the app does, the button/
      spacebar cycle and long-press clear, how to pick your ASIO device.

## v2 backlog (explicitly deferred)

- Trim loop start/end
- Undo (separate from full clear)
- Overdub — recorded as separate removable/mute-able layers rather than
  mixed into one buffer (see "Overdub as layers" under v3 below); the
  layer abstraction is shared with the v3 multitrack idea, so worth
  building once, generically, instead of solving overdub and multitrack
  separately
- ~~Loop playback volume~~ - done: a Settings slider (0-200%), persisted
  in `AppConfig`, applied via `SharedControl::volume_pct` to the loop
  signal only (not the live passthrough) in the output callback's mix
  step
- NAM model loading (insert-effect stage)
- Save loop to file
- Tempo sync / metronome
- Waveform rendering

## UI/framework decoupling (before or alongside v2 UI growth)

Today `main.rs` mixes screen state, input handling, and egui rendering
together. Before adding more screens (waveform, trim handles, a
per-layer track list), split a thin view-model out of the egui-specific
rendering:

- `Screen`/`SettingsState`/`LooperState` become plain data + methods
  (`press()`, `select_device()`, `start()`, ...) with no `egui::` types
  in them.
- `ui/*.rs` functions stay pure `fn render(ui: &mut egui::Ui, model: &Model) -> Option<Action>`
  - the pattern `state_indicator` already follows.

This buys cheap framework-independence (egui -> another native Rust GUI
lib, if ever wanted) without committing to anything. It does **not** buy
a web port on its own - see below.

### Can this run on the web?

Two separable questions:

- **UI**: egui/eframe already supports compiling to WASM for a browser
  target - the *rendering* code could run in a browser mostly unchanged,
  once it's decoupled from the audio engine per above.
- **Audio**: `cpal`'s ASIO backend is Windows-only with no browser
  equivalent. A real "runs in the browser" looper needs a from-scratch
  Web Audio API (AudioWorklet) engine - not a port of `engine.rs`, a
  parallel implementation of the same state machine and buffer logic
  against a different real-time audio API. That's a separate project,
  not a refactor.
- **Middle ground**, if a "web version" is ever wanted for real: keep
  the native app as the audio engine, and build a browser page as a
  *remote control/monitor* UI talking to it over a local socket (state +
  meters out, button presses in) instead of doing audio in the browser
  at all.

Recommendation: do the view-model split because it's cheap and improves
clarity regardless of platform; treat an actual browser audio engine as
its own future project rather than a backlog item on this one.

## CI/CD and other platforms (before v2)

Requested before v2 work starts, alongside the review-driven fixes below.

### GitHub Actions: build + release (Windows, now)

Added as `.github/workflows/release.yml`: builds + tests on every push/PR,
and on a `v*.*.*` tag, zips the release exe and attaches it to a GitHub
Release. One real unknown, flagged in the workflow's own comments: it
points `CPAL_ASIO_DIR` at a fresh clone of the GPLv3 ASIO SDK fallback
(`github.com/audiosdk/asio`) rather than relying on `asio-sys`'s own
auto-download, since that path is more likely to need something
interactive that won't work headless. This needs a live run to confirm
- can't be verified without pushing.

*Licensing note, worth being aware of before distributing a binary to
anyone else:* the GPLv3 ASIO SDK fallback is what CI (and anyone without
a Steinberg license) builds against. Depending on how `asio-sys` links
the compiled shim, that can carry GPL obligations onto the distributed
binary. Not a concern for building/running it yourself, but worth a
second look before treating a GitHub Release as "anyone can download
and use this."

### Other platforms - honest effort/risk read

- **macOS**: moderate effort. `cpal` already has a CoreAudio backend, so
  the main work is (1) making host selection platform-conditional
  instead of hardcoding `HostId::Asio` in `engine.rs`'s `asio_host()`,
  and (2) generalizing the i32-sample-format assumption baked in
  throughout `engine.rs` (documented there as tied specifically to the
  Audient iD4 MkII) - CoreAudio devices commonly report f32 instead.
  Realistic to do once the UI/framework decoupling above is in place.
- **Linux**: moderate-to-higher effort, same host-abstraction and
  sample-format work as macOS, plus more backend fragmentation to
  navigate (ALSA vs JACK vs PipeWire) and generally less predictable
  low-latency behavior out of the box.
- **Android / iOS**: a materially bigger, riskier undertaking, not just
  "one more platform" - `cpal`'s mobile backend support is limited/less
  battle-tested than desktop, `egui`/`eframe`'s mobile support is
  rougher (especially iOS), and mobile OSes make the one guarantee this
  whole app is built around - low, predictable audio latency - much
  harder to get consistently. Recommend a small, throwaway research
  spike (can a minimal cpal+eframe passthrough even hit usable latency
  on a real phone?) before this goes anywhere near the backlog as a
  planned feature, rather than assuming it's parity with a desktop port.

Suggested order: Windows CI now -> macOS port -> Linux port -> mobile
only if a spike confirms it's viable.

### Auto-versioning

Every push to `main` (from a human, not the bot itself) auto-bumps the
patch version in `Cargo.toml`/`Cargo.lock`, commits it, tags it
`vX.Y.Z`, builds the release binary against that new version, and
publishes it as a GitHub Release - all handled by `release.yml`. A
couple of things worth knowing:

- Every commit to `main` becomes a tagged release. If that turns out to
  be too noisy once things stabilize, the easy next step is gating it
  behind something explicit (e.g. only bump when the commit message
  contains a marker) rather than every push.
- The bot commits/pushes directly to `main` using the workflow's own
  token - this will fail if branch protection on `main` requires PR
  review. Worth checking before the first push if that's configured.
- Minor/major bumps are still manual - the automation only ever
  increments the patch number, since there's no commit-message
  convention (e.g. conventional commits) in place to infer bigger
  bumps from.

## v3: extended multitrack version (separate, still minimal)

Vision: still "run and play," not a DAW. Preconfigured mic + guitar
input tracks, ability to add a few more, metronome, drum presets, tuner
- kept as small and single-purpose as the mini looper is today. Two
builds are expected to coexist: `mini` (today's single-track looper,
unchanged) and `extended` (this).

- **Track abstraction**: overdub-with-layers (v2, above) and multitrack
  are the same underlying data model - a small stack of aligned
  buffers, each independently record/mute/delete-able, rather than one
  flat `LoopBuffer`. Build it once as the shared abstraction both
  features sit on top of.
- **Metronome**: a click scheduled off sample position + BPM, mixed in
  like a track; pure/testable scheduling logic, same style as the state
  machine.
- **Drum presets**: pre-recorded sample loops played back as another
  track type (sample-backed instead of live-recorded) - reuses the
  track abstraction rather than a separate playback path.
- **Tuner**: pitch detection on the live input signal. Must not run
  inside the real-time callback itself (autocorrelation/FFT cost is
  unbounded relative to the audio budget) - publish a decimated copy of
  input samples through a lock-free channel and analyze on a background
  thread or per UI frame, same pattern as `SharedControl`'s existing
  telemetry.
- **Mini stays mini**: e.g. a Cargo workspace with a shared core crate
  (audio engine, track/loop buffers, state machine) and two thin
  binaries (`mini`, `extended`) that each assemble a different
  UI/feature set on top, so the simple version never carries multitrack
  code paths it doesn't use.

## Documentation update (after v2)

Once any v2 item above is implemented, update both docs (technical +
user) to reflect it — docs stay in sync with whatever's actually built,
not frozen at the MVP.
