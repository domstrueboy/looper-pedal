# Guitar Looper - Plan

A minimal, single-track looper pedal replacement for practicing guitar
through an ASIO audio interface. Standalone Windows app - no DAW, no
plugin host.

This file is forward-looking: what's left to build, in what order, and
the decisions that shouldn't drift. What already exists is documented
where it belongs instead of being restated here - `README.md` for
architecture, module layout and build steps, `docs/user-guide.md` for
how the app is actually used.

## Status

v0.1.2, MVP complete. Working today: an ASIO device / sample-rate /
input-channel picker with persisted config, always-on live monitoring,
the 4-state record -> loop -> stop -> resume cycle on spacebar or button,
~2s long-press clear from any state, state indicator with loop length
and playback position, loop playback volume, technical + user docs, and
a Windows CI build/release pipeline. Everything below is what's left.

## Ground rules

- **The real-time boundary is the one thing that must not drift:** no
  locks and no allocations on the audio thread. UI <-> audio talks only
  through `SharedControl`'s atomics or a ring buffer, never a mutex.
- **Stays small:** "run and play", not a DAW. No upfront abstraction and
  no UI framework layer - small functions and structs, factored further
  only when duplication actually shows up.
- **Minimal dependencies,** with v2 step 6 as the one deliberate
  exception.
- **`Renderer::Glow` is mandatory** - the default wgpu renderer crashes
  (STATUS_ACCESS_VIOLATION) on this machine's Intel UHD graphics.
- **i32 sample format** is assumed throughout `engine.rs` (the Audient
  iD4 MkII's native format); generalizing it is part of any non-Windows
  port.
- **One small commit per step,** so history stays reviewable step by
  step rather than as one large diff.

## v2 roadmap (ordered)

### 1. UI/view-model split - done

`main.rs` is eframe glue only now. `app.rs` (which screen is up, acting
on actions), `looper.rs` and `settings.rs` hold the state as plain data
with no `egui::` types, and each pairs with a renderer of the same name
under `ui/` following `render(ui, model) -> Option<Action>`. See
README's module layout.

Step numbers below stay fixed as items land - they're cross-referenced
throughout this file.

### 2. Overdub as removable layers

Not one flat `LoopBuffer` but a small stack of aligned buffers, each
independently record/mute/delete-able. This is the same data model v3's
multitrack needs, so build it once, generically, instead of solving
overdub and multitrack separately.

### 3. Hide the console window outside dev builds

The black terminal behind the app UI is just cargo's default
console-subsystem binary hosting stdout/stderr - nothing in the app
opens it deliberately. Fix:
`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` in
`main.rs`, so release builds get no console while `cargo run` still
does. That's "dev mode only" with no runtime flag to pass.

Catch: it also silences the diagnostics a user could act on - the
underrun warnings and stream errors. See Open decisions.

### 4. Pre-roll delay before recording

An optional, configurable wait between pressing record and actually
capturing, so there's time to get hands back on the guitar. A 0-5s
slider in Settings, persisted like the other fields; 0 means today's
behavior.

Add an `Arming` state advanced by the UI frame loop, which already
ticks every frame and already works in `Instant`. The audio thread
keeps seeing `Idle` until the state flips to `Recording`, so no timing
code goes into the callback and sample accuracy isn't needed for a "get
ready" window. A press while arming cancels back to `Idle`; the
indicator needs an `Arming` row counting down and a cancel icon.

Build it as a general arming window - v3's metronome count-in is the
same window with clicks in it.

### 5. Preserve the loop across restarts

The recorded loop should survive closing the app. Not a user-facing
save feature yet, but stored so that export later is a file copy plus
the same writer rather than a converter: write a mono 32-bit-int PCM
WAV (a ~15-line header, no new dependency, self-describing).

The hard part is access, not format - `LoopBuffer` lives exclusively
inside the output callback. Push recorded samples through a second ring
buffer that the UI thread drains into a plain `Vec`, the same lock-free
audio -> UI direction the existing telemetry uses; 60s of mono i32 at
48 kHz is ~11 MB. (Snapshotting the buffer under a lock is rejected -
no locks within reach of the callback.) Loading is the easy half:
`build_looper_streams` takes an optional pre-filled buffer, since the
buffer is handed to the callback at build time anyway.

Semantics: restore into `Stopped`, never auto-playing on launch.
Discard the stored loop if its sample rate doesn't match the stream
being opened - no resampling. Long-press-clear drops the stored copy
too. Write on exit, not per frame.

### 6. Settings in a real config file

Replace `config.rs`'s hand-rolled `key=value` parser with TOML (`serde`
+ `toml` - comments are allowed and it's what the Rust ecosystem
speaks), stored in the per-user config directory via `directories`
instead of next to the executable:

- Windows: `%APPDATA%\looper-pedal\config.toml`
- Linux: `~/.config/looper-pedal/config.toml`
- macOS: `~/Library/Application Support/looper-pedal/config.toml`

Next to the exe stops being writable the moment the app is installed
somewhere like Program Files, and step 5 needs a per-user directory
anyway - so do this before or together with 5, rather than writing loop
state next to the exe and then moving it.

This is the one place the minimal-deps rule is deliberately traded away
(`serde`, `toml`, `directories`): the bespoke parser stopped paying once
the field count grew. Optionally migrate an existing `looper-pedal.cfg`
once, so the saved device choice survives the move.

### 7. Mic channel, cheap subset (optional)

Only worth doing if practicing with vocals is wanted before the
extended build exists. Capture two chosen input channels instead of
one, each with its own gain, summed to mono and recorded as a single
loop. `LoopBuffer` stays mono and no track abstraction is needed;
Settings grows a second channel picker and two gain sliders.

Nothing stands in the way: both iD4 inputs are on the same stream and
the input callback already receives every channel interleaved - it just
discards all but `input_channel`. So there's no second device to open
and no cross-device drift.

Two things the single-channel path never had to face: **per-source
monitoring** (guitar monitoring is always wanted, live mic monitoring
often isn't - feedback, and hearing yourself dry is unpleasant), and
**levels that differ enormously** between an instrument input and a mic
preamp, so one shared gain isn't enough - eventually a simple input
meter, so it can be set by eye.

The full version - mic and guitar as separate tracks with independent
record-enable, mute and level - is v3, once step 2's layer abstraction
exists.

## Open decisions

Worth settling before the work they block starts.

- **Loop / bar-grid sync** - blocks both the metronome and the drum
  machine. A loop recorded today is whatever length happened to be
  played, so it won't line up with a bar grid and drums against it will
  clash. Either quantize the loop length to a whole bar when recording
  stops (needs the clock running *while* recording, and pairs naturally
  with step 4's arming window as a count-in), or leave the drums
  free-running and keep time yourself. Recommend quantizing - decided
  before either feature is built, since both depend on it.
- **Release-build diagnostics** - once step 3 removes the console, do
  underrun warnings and stream errors stay debug-only (a release build
  is then silent), or get a small in-app surface such as a status line
  under the indicator? In-app is the better long-term answer, as its
  own small UI task.
- **Mic in `mini` or `extended` only** - step 7 above, or wait for real
  tracks in v3.
- **GPL ASIO SDK before sharing binaries** - CI builds against the
  GPLv3 fallback SDK, and depending on how `asio-sys` links its
  compiled shim that can carry GPL obligations onto a distributed exe.
  Irrelevant for building and running it yourself; worth a look before
  treating a GitHub Release as "anyone can download this".

## Later (unordered)

- Trim loop start/end, and the waveform rendering it needs anyway
- Undo, distinct from full clear
- Export / save loops to a chosen file - reuses step 5's WAV writer
- NAM model loading, in the insert-effect stage the signal path leaves
  room for between input and the mix
- Tuner - pitch detection on a decimated copy of the input, analysed on
  a background thread or per UI frame. Never inside the callback: FFT /
  autocorrelation cost is unbounded relative to the audio budget.
- **macOS port** - moderate effort: make host selection conditional
  instead of hardcoding `HostId::Asio`, and generalize the i32
  assumption (CoreAudio commonly reports f32). Realistic once step 1 is
  done.
- **Linux port** - the same work, plus backend fragmentation (ALSA vs
  JACK vs PipeWire) and less predictable low-latency behavior.
- **Mobile** - not "one more platform". `cpal` and `eframe` are both
  rougher there, and mobile OSes make the low, predictable latency this
  app is built around much harder. Needs a throwaway spike answering
  whether a minimal cpal + eframe passthrough can even hit usable
  latency on a real phone, before it becomes a backlog item.
- **Web** - egui already compiles to WASM, but `cpal`'s ASIO backend has
  no browser equivalent, so a real web version means a parallel
  AudioWorklet engine - a separate project, not a port. Cheaper middle
  ground if ever wanted: keep this app as the audio engine and serve a
  browser page as a remote control/monitor over a local socket.

## v3: extended build

Still "run and play", not a DAW: preconfigured mic + guitar tracks, a
few more if wanted, metronome, drum machine, tuner - kept as small and
single-purpose as the mini looper is today.

- **Track abstraction** - step 2's layer stack, reused rather than
  reinvented.
- **The metronome is a one-voice drum machine.** Build one scheduler -
  (sample position, BPM, pattern) -> events -> voices, pure and
  testable in the style of the state machine - and the metronome is a
  preset of it rather than a separate feature with its own timing code.
- **Drum machine** - one-shot samples plus an editable step pattern,
  not pre-recorded loops (whose tempo is baked in, so changing BPM would
  mean resampling). A small fixed-size polyphonic voice pool,
  pre-allocated to stay RT-safe. Kits load on the UI thread and are
  handed to the audio thread the way step 5 hands over a restored loop,
  never loaded in the callback; embed one small CC0 kit via
  `include_bytes!` to keep the single-exe property, with an optional
  folder next to the config for more.
- **Mini stays mini** - a Cargo workspace with a shared core crate
  (audio engine, buffers, state machine) and two thin binaries (`mini`,
  `extended`) assembling different feature sets, so the simple build
  never carries multitrack code paths it doesn't use.

## CI / releases

`.github/workflows/release.yml`: every push to `main` bumps the patch
version, commits and tags it, builds on `windows-latest` against the
GPLv3 ASIO SDK, and attaches a zip to a GitHub Release - one job, one
run. It deliberately doesn't rely on the tag push triggering a second
run, since pushes made with the default `GITHUB_TOKEN` don't trigger
workflows.

Confirmed working: the bump/commit/tag/push half - the bot produced
both `v0.1.1` and `v0.1.2`, so branch protection isn't in the way.
Still unverified: the bump step runs *before* the build, so those tags
don't prove the headless ASIO SDK fetch, the tests, the release build
or the zip upload succeeded. Worth checking whether those two releases
actually have the zip attached.

Known rough edges: every commit to `main` becomes a tagged release
(gate it behind a commit-message marker if that gets noisy), and only
patch bumps are automatic - minor/major stay manual, since there's no
commit convention to infer them from.

## Docs

As each item lands, update `README.md` (architecture, module layout,
build) and `docs/user-guide.md` (usage) - docs track what's actually
built rather than staying frozen at the MVP.
