# Looper Pedal

A minimal single-track looper pedal replacement for practicing guitar
through an ASIO audio interface - one loop, stacked from overdub
layers. Standalone Windows app - no DAW, no plugin host. See `docs/user-guide.md` for how to use it; this document
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

Both callback bodies are `InputPath::process` and `OutputPath::process`
rather than closure bodies, so the whole path can be driven by a test
without opening a device - which is how the recording alignment below is
measured.

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
  config.rs                  the settings, as TOML in the per-user
                              config dir (config_tests.rs)
  build.rs                   generates the .ico from `ui/icon.rs` and
                              embeds it as the exe's icon resource
  input.rs                   short-press vs long-press-clear detection
                              (input_tests.rs)
  state_machine.rs           the pedal logic - pure, no audio and no
                              egui (state_machine_tests.rs)
  loop_mirror.rs             the UI thread's own copy of the loop, and
                              saving it (loop_mirror_tests.rs)
  preroll.rs                 the countdown before the first recording
                              (preroll_tests.rs)
  wav.rs                     mono 32-bit PCM, by hand (wav_tests.rs)
  audio/
    engine.rs                device enumeration, config negotiation, the
                              actual cpal streams and audio callbacks
    shared_control.rs         lock-free UI <-> audio thread relay
    loop_stack.rs              pre-allocated stack of aligned mono loop
                              layers (loop_stack_tests.rs)
  ui/                        rendering only - each screen is a
                              `render(ui, model) -> Option<Action>` fn
    looper.rs                 the looper screen
    settings.rs               the settings screen
    indicator.rs              state circle + label + progress bar widget
    icon.rs                   the app icon, drawn as RGBA at any size
                              (icon_tests.rs)
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

**Long-press**, from any state, clears the loop and returns to Idle - it
fires the moment the hold crosses the threshold while still held, not on
release. How long that hold is is the `long_press_ms` setting, 2s by
default, and the looper screen's hint line says the configured value.

### Pre-roll

With a **record delay** set in Settings (5s by default), step 1 goes
through **Arming** instead: a bar fills as the wait runs out, then
Recording. It's for the first recording only - once a loop is playing
you're already in time with it - and a press part-way through calls it
off, back to Idle.

The countdown runs on the UI thread, which already ticks every frame and
already works in `Instant`, so no timing code goes anywhere near the
audio callback: the callback sees `Arming` and captures nothing, exactly
as it does for `Idle`, until the state flips to `Recording`. `Arming` is
still a published state of its own rather than being hidden from the
audio thread, because a metronome count-in eventually needs to click
during that window. Input is handled before the countdown each frame, so
a press meant to cancel wins over the countdown happening to run out on
the same frame.

**Overdub** deliberately sits *off* that cycle, on its own control (the
`O` key or the Overdub button), so the press cycle above keeps behaving
as the four states above: **Looping** <--overdub--> **Overdubbing**. A
press while overdubbing stops playback, like it does while looping - the
main control always means "stop" when something is playing.

### Layers

The first recording fixes the loop length; each overdub adds another
layer on top, and playback is their sum. `max_layers` of them can be
stacked, four by default, and the newest can be dropped again ("Remove
last") - dropping the only one is the same thing as clearing.

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

Stacked takes can sum past what an `i32` sample holds, so layers are
summed in 64-bit and the loop volume is applied to that full-precision
sum before it's clamped back down (`scale_and_clamp`). Clamping first
would make a hot stack permanently crunchy; this way turning the loop
volume down still recovers it.

### Recording alignment

The dry passthrough carries `latency_ms` of headroom between the two
callbacks - that headroom is what absorbs their jitter - so you hear
yourself that much after the fact. The recorded signal is held back by
the same amount (`MonitorDelay`) before it reaches either the layer
stack or the UI thread's copy. Without that, the sample stored at loop
position p would be a whole monitoring delay fresher than the one heard
at p: takes would land ahead of the beat they were played against, and
every layer stacked on top would inherit the error again. An
`engine_tests.rs` measurement drives both callbacks with a ramp and
asserts the offset is zero.

The rings hold `latency_frames + SCRATCH_CAPACITY` so a driver buffer
larger than the delay still fits - the prefill, not the capacity, is
what sets the delay.

## Settings & persistence

On first run (or if the saved config no longer opens - e.g. the interface
was unplugged), the app shows a Settings screen: pick the ASIO device,
sample rate and input channel, and set anything in the table below. On
"Start" this is saved and the app launches straight into the looper on
subsequent
runs. The gear icon (top-right, in the looper screen) reopens Settings at
any time, pre-selecting whatever's currently active.

Settings are TOML in the per-user config directory:

- Windows: `%APPDATA%\looper-pedal\config.toml`
- Linux: `~/.config/looper-pedal/config.toml`
- macOS: `~/Library/Application Support/looper-pedal/config.toml`

Next to the executable - where they used to live - stops being writable
the moment the app is installed somewhere like Program Files, and the
recorded loop will want a per-user directory of its own soon anyway.

`device_name` and `sample_rate` are required; everything else falls back
to a default, so a config written by an older build still loads instead
of throwing you back to the settings screen. Unknown keys are ignored,
and comments are allowed, so the file is safe to hand-edit.

Every setting's default and allowed range is declared once, in
`config.rs`: the settings screen builds its sliders from those ranges,
and a hand-edited file is clamped to them on load. Nine hundred layers
of a two-hour loop would otherwise try to allocate terabytes before the
window opened.

| Setting | Default | Range | Effect |
|---|---|---|---|
| `volume_pct` | 100 | 0-200 | loop playback gain, live signal untouched |
| `preroll_ms` | 5000 | 0-5000 | wait before recording starts; 0 = off |
| `long_press_ms` | 2000 | 500-4000 | how long a hold clears the loop |
| `latency_ms` | 8 | 2-50 | callback headroom - raise it if the log reports underruns |
| `max_loop_secs` | 60 | 10-120 | longest recordable loop |
| `max_layers` | 4 | 1-8 | layers including the first recording |

The last two are a memory multiplier - every layer is pre-allocated at
the full loop length, so it costs `seconds x layers x sample rate x 4`
bytes, and the settings screen shows the figure next to the sliders.

Deliberately *not* settings: `SCRATCH_CAPACITY` (the bound on one
callback - what a driver might hand us, not how anyone wants the app to
behave; it sizes the scratch buffers and the headroom in the rings),
`CANDIDATE_SAMPLE_RATES` (a probe list, not a choice), and the window
and widget sizes.

## Saved loops

The recorded loop survives closing the app: one WAV per layer, beside the
config -

- Windows: `%APPDATA%\looper-pedal\loop\layer-1.wav`, `layer-2.wav`, ...

Mono 32-bit PCM, written by hand (`wav.rs`) rather than through a
dependency. They're deliberately ordinary files: any editor opens them,
and the eventual "export loop" feature becomes a copy rather than a
converter.

Written whenever the loop changes, not on the way out, so a crash or a
kill doesn't lose it. On launch the layers come back and the looper
starts **Stopped** - the loop is there, silent until asked for. A saved
loop is discarded rather than adapted if it was recorded at a different
sample rate (there's no resampling) or is longer than `max_loop_secs`
now allows; layers past `max_layers` are dropped on their own, the ones
underneath being independent takes. `loop_mirror::load` decides all of
that in one place, so the layer stack and the UI thread's copy are
always seeded from the same list.

### Why a second copy exists

`LoopStack` lives inside the output callback, where nothing may reach in
and read it - so the UI thread keeps its own copy (`loop_mirror.rs`) to
have something it can write. The input callback pushes captured samples
into a second ring buffer alongside the recorder's, and the UI thread
drains it each frame. Same lock-free audio -> UI direction as the
telemetry; no lock anywhere near the callback. Both rings are fed from
the same delayed signal (see below), so the two layouts describe the
same audio.

Takes are stored as they were played - where each began, and the samples
- and only laid out into full-length layers when saving, so the copy
costs about what was recorded rather than a second full-size stack. Two
consequences worth knowing:

- **The overdub layout rule exists twice**: the audio thread applies it
  sample by sample as it plays, the mirror applies it to a whole take at
  once. A test asserts the two agree, because drift between them would
  mean a loop that reloads sounding different from the one that was
  played.
- **If samples ever go missing** on the way to the copy (its ring
  overflowing because the UI stalled), the save is skipped rather than
  writing a loop that doesn't match what was heard. The count comes back
  through `SharedControl` like the underrun counters.

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
cargo test        # run the unit tests - no device needed, including the audio path
```

The app icon is drawn in code (`ui/icon.rs`) rather than stored as an
image: a green loop arrow around a red record dot. That means no
image-decoding dependency and no binary asset in the repo, and it
renders at any size - the window asks for one, and `build.rs` `include!`s
the same file to generate a multi-size `.ico` for the executable's icon
resource. Embedding needs `rc.exe` from the Windows SDK; if it's missing
the build warns and carries on without the exe icon.

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
