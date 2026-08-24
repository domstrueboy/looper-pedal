# Looper Pedal

A minimal, single-track looper pedal replacement for practicing guitar
through an ASIO audio interface. Standalone Windows app - no DAW, no
plugin host. See `docs/user-guide.md` for how to use it; this document
covers the architecture and how to build it.

## Architecture

The app owns the ASIO device directly (input + output) rather than being
a plugin, since ASIO devices are typically single-client. Audio flows
through a lock-free pipeline entirely within `src/audio/`, driven by a
state value published from the UI thread.

```
Guitar -> ASIO in (selected channel only) -> live monitor (always audible)
                                           -> mixed with loop playback
                                           -> duplicated to every ASIO out
```

Only the input channel chosen in Settings is captured - it's treated as
mono internally (recorded, looped) and duplicated equally across every
output channel, so a single guitar input is centered in both ears rather
than only coming out of one side. NAM (Neural Amp Modeler) integration is
explicitly out of scope for now - the signal is clean/dry throughout.

### Thread boundary

The real-time audio callback stays strictly separate from UI/state logic:
no locks or allocations on the audio thread. State changes (button
presses, mode transitions) and telemetry (loop length, playback position)
cross the thread boundary via `SharedControl`, which is nothing but a
handful of atomics - no mutex anywhere in the audio path.

- **UI thread** owns `LoopStateMachine` (the actual state) and publishes
  its value into `SharedControl` whenever it changes.
- **Audio thread** (the output callback) owns `LoopBuffer` exclusively -
  it's never shared with the input callback. The input callback only
  reads `SharedControl`'s published state to decide whether to feed
  captured samples toward the recorder.

## Module layout

```
src/
  main.rs                    app entry, screen switching (Settings <-> Looper),
                              egui window setup, spacebar/button input wiring
  config.rs                  persisted device/rate/input-channel choice
                              (looper-pedal.cfg next to the exe)
  input.rs                   short-press vs long-press-clear detection
                              (input_tests.rs)
  audio/
    engine.rs                device enumeration, config negotiation, the
                              actual cpal streams and audio callbacks
    shared_control.rs         lock-free UI <-> audio thread relay
    loop_buffer.rs             pre-allocated mono ring buffer for the
                              recorded loop (loop_buffer_tests.rs)
    state_machine.rs           the 4-state pedal logic, pure/no audio
                              (state_machine_tests.rs)
  ui/
    indicator.rs              state circle + label + progress bar widget
```

Tests live in sibling `*_tests.rs` files (via `#[path = "..."] mod tests;`)
rather than inline, to keep the implementation files themselves short -
see any of the pairs above. Run them with `cargo test`.

## State machine

Mimics a classic single-footswitch looper pedal:

1. **Idle** (empty) --press--> **Recording**
2. **Recording** --press--> **Looping** (recording stops, loop length is
   fixed, playback starts immediately and loops seamlessly)
3. **Looping** --press--> **Stopped** (playback silenced, loop stays in
   memory)
4. **Stopped** --press--> **Looping** (resumes the same loop)

**Long-press (~2s hold)**, from any state, clears the loop and returns to
Idle - it fires the moment the hold crosses the threshold while still
held, not on release.

## Settings & persistence

On first run (or if the saved config no longer opens - e.g. the interface
was unplugged), the app shows a Settings screen: pick the ASIO device,
sample rate, and which input channel to use. On "Start" this is saved to
`looper-pedal.cfg` next to the executable and the app launches straight
into the looper on subsequent runs. The gear icon (top-right, in the
looper screen) reopens Settings at any time, pre-selecting whatever's
currently active.

## Build prerequisites (Windows)

- Rust via `rustup`, MSVC toolchain (`x86_64-pc-windows-msvc`)
- Visual Studio Build Tools - "Desktop development with C++" workload
  (needed for linking, and for compiling the ASIO SDK's C++ shim)
- LLVM/libclang (needed by `bindgen`, which `asio-sys` uses to generate
  bindings to the ASIO SDK headers)
- Steinberg ASIO SDK - dual-licensed (GPLv3 or proprietary) since Oct
  2025; `asio-sys` (used by `cpal`) auto-downloads it during
  `cargo build` if `CPAL_ASIO_DIR` is left unset, so there's normally
  nothing to fetch by hand

## Building & running

```powershell
cargo build       # compile
cargo run         # build + launch
cargo test        # run the unit tests (state machine, loop buffer, input handler)
```

The project targets a single specific device family (asserts an i32
sample format), since it's built around one Audient iD4 MkII - other
ASIO interfaces that also report i32 should work, but this hasn't been
tested against others.
