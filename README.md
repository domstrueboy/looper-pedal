# Looper Pedal

A minimal single-track looper pedal replacement for practicing guitar
through an ASIO audio interface - one loop, stacked from up to four
overdub layers. Standalone Windows app - no DAW, no
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
                                              (sum of all layers)
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
- **Audio thread** (the output callback) owns `LoopStack` exclusively -
  it's never shared with the input callback. The input callback only
  reads `SharedControl`'s published state to decide whether to feed
  captured samples toward the recorder.

The callback only ever sees the published state *value*, never the
transitions, so it detects those itself by comparing against the state it
saw last: that's when the loop length gets fixed and an overdub layer is
opened or closed.

## Module layout

```
src/
  main.rs                    eframe glue and nothing else: window setup,
                              and handing each frame to a screen renderer
  app.rs                     which screen is up, screen switching, and
                              acting on what a renderer reports back
  looper.rs                  looper-screen state: state machine, the audio
                              relay, the live streams, per-frame tick
  settings.rs                settings-screen state: device/rate/channel
                              lists and what's selected
  config.rs                  persisted device/rate/input-channel choice
                              (looper-pedal.cfg next to the exe)
  input.rs                   short-press vs long-press-clear detection
                              (input_tests.rs)
  audio/
    engine.rs                device enumeration, config negotiation, the
                              actual cpal streams and audio callbacks
    shared_control.rs         lock-free UI <-> audio thread relay
    loop_stack.rs              pre-allocated stack of aligned mono loop
                              layers (loop_stack_tests.rs)
    state_machine.rs           the pedal logic, pure/no audio
                              (state_machine_tests.rs)
  ui/                        rendering only - each screen is a
                              `render(ui, model) -> Option<Action>` fn
    looper.rs                 the looper screen
    settings.rs               the settings screen
    indicator.rs              state circle + label + progress bar widget
```

`app.rs`, `looper.rs` and `settings.rs` hold no `egui::` types at all, so
the app model isn't tied to the library drawing it - `main.rs` and `ui/`
are the only framework-aware parts. Each model file pairs with a renderer
of the same name under `ui/`.

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

**Overdub** deliberately sits *off* that cycle, on its own control (the
`O` key or the Overdub button), so the press cycle above keeps behaving
exactly as it always has: **Looping** <--overdub--> **Overdubbing**. A
press while overdubbing stops playback, like it does while looping - the
main control always means "stop" when something is playing.

### Layers

The first recording fixes the loop length; each overdub adds another
layer on top, and playback is their sum. Up to `MAX_LAYERS` (4) can be
stacked, and the newest can be dropped again ("Remove last") - dropping
the only one is the same thing as clearing.

Each layer is a full-length pre-allocated buffer, but an overdub can
start anywhere in the loop and be stopped early, so a layer only counts
as recorded over the window it was actually played into (`written`
samples from `start`). Outside that window it's never read, which is what
lets a layer be reused without memsetting megabytes inside the audio
callback. Two consequences worth knowing:

- A take is mixed in *before* the incoming sample is written over it, so
  you never hear the pass you're currently playing echoed back on top of
  your live signal - it comes back from the next pass onward.
- Overdubbing past the end of the loop sums into the same layer rather
  than replacing it, so a second pass doesn't erase the first.

### Layer volume

There is no per-layer gain: every layer plays back at the level it was
recorded at, and Settings' single "Loop volume" is the only control over
them - it scales the whole stack, still leaving the live passthrough
alone.

Four stacked takes can sum past what an `i32` sample holds, so layers are
summed in 64-bit and the loop volume is applied to that full-precision
sum before it's clamped back down (`scale_and_clamp`). Clamping first
would make a hot stack permanently crunchy; this way turning the loop
volume down still recovers it.

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
cargo test        # run the unit tests (state machine, layer stack, input handler)
```

Debug builds are console-subsystem binaries, so `cargo run` keeps a
terminal alongside the window - that's where the stream config, underrun
warnings and stream errors print. Release builds set
`windows_subsystem = "windows"` and have no console at all, which also
means those diagnostics go nowhere; run a debug build when chasing
audio trouble.

The project targets a single specific device family (asserts an i32
sample format), since it's built around one Audient iD4 MkII - other
ASIO interfaces that also report i32 should work, but this hasn't been
tested against others.
