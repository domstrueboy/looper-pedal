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

**v2 is complete.** Working today: an ASIO device / sample-rate /
input-channel picker with settings persisted as TOML in the per-user
config directory, always-on live monitoring, the record -> loop -> stop
-> resume cycle on spacebar or button, overdub layers on a control of
their own with remove-last, a configurable pre-roll before recording,
long-press clear, the loop surviving restarts as WAV layers, an app
icon, no console window in release builds, technical + user docs, and a
Windows CI build/release pipeline.

The post-v2 review below has been carried out. It found four
correctness faults - two in the saved-loop path, two in the recording
path - and left the module boundaries clean enough for v3's workspace
split to be drawn around them. What it changed is in README and in the
history; what it left open is under that heading.

Next is v3, which needs ordering before it can start.

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

### 2. Overdub as removable layers - done

`LoopStack` replaced the flat `LoopBuffer`: the first recording fixes the
loop length and each overdub adds an aligned layer on top, up to
`MAX_LAYERS` (4), played back as their sum. The newest layer can be
dropped again. Overdub is on its own control (`O` / a button) rather than
in the press cycle, so nothing about the existing cycle changed - see
README's state machine and layers sections.

Per-layer **mute** is the one part of "record/mute/delete-able" not built:
it needs the per-layer UI list, which the count-plus-remove-last UI
deliberately skipped. Worth doing together with the waveform work, or
whenever a layer list appears.

### 3. Hide the console window outside dev builds - done

`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` in
`main.rs`: the release binary is a GUI-subsystem exe with no console,
`cargo run` still builds a console one. Verified by reading the
subsystem field out of both binaries' PE headers (2 vs 3).

`asio_host()` went with it: it used to `expect()`, and a panic before
the window opens is invisible without a console, so it now reports and
the settings screen shows it.

### 4. Pre-roll delay before recording - done

An `Arming` state advanced by the UI frame loop, with a 0-5s "Record
delay" slider in Settings, defaulting to 5s and reading "off" at zero.
The wait shows as the seconds counting down where the recording time
normally appears, over a light gray bar filling beneath it - laid out
like the looping and recording readouts rather than as its own thing. A
lone number on the button read as ambiguous, and a bar's own `text` sits
on the left in egui, where a shrinking number behind a growing fill
reads as a contradiction. First recording only; a press part-way through
calls it off, and the button shows a cancel cross to say so. The audio
callback treats `Arming` exactly like `Idle`, so no timing code went
near it - see README's pre-roll section.

`Arming` is published rather than hidden from the audio thread on
purpose: v3's metronome count-in is this same window with clicks in it,
and will need the callback to know.

`Action::Start` now carries an `AppConfig` rather than a growing list of
fields, and `AppConfig::load` defaults a missing `preroll_ms` instead of
rejecting the file, so configs written before this still load.

### 5. Preserve the loop across restarts - done

One WAV per layer beside the config, and a copy of the loop kept on the
UI thread to have something to write - see README's saved-loops section
for how and why.

Two decisions changed while building it:

- **Saved when the loop changes, not on exit.** An exit hook would have
  needed the whole loop pulled out of the audio thread in one moment,
  which the streaming handoff can't do; saving on change is also
  proof against a crash.
- **Per-layer, not mixed down.** Once the UI thread holds the takes,
  writing them separately costs nothing extra and keeps "remove last"
  meaningful after a restart.

Take layout is implemented twice - incrementally in the callback, in
one go in the mirror - with a test asserting they agree. Worth folding
into one implementation if a third caller ever appears.

### 6. Settings in a real config file - done

TOML via `serde` + `toml`, in the per-user config directory found with
`directories` - see README's settings section for the paths and the
compatibility rules. `device_name` and `sample_rate` are required,
everything else defaults, so adding a setting can't invalidate an
existing file. The reader for the old `looper-pedal.cfg` next to the
executable was dropped in the post-v2 review, no such file being likely
to be left anywhere.

Deps added as planned, the one deliberate exception to minimal-deps:
`serde`, `toml`, `directories`.

Step 5 can now put the recorded loop next to the config rather than next
to the exe.

Settings that used to be constants moved into the config and onto the
screen at the same time: latency, max loop length, max layers and the
hold-to-clear time. Each one's default and range live together in
`config.rs`, the sliders are built from those ranges, and a hand-edited
file is clamped to them - see README's settings table. The screen scrolls
now, with Start pinned below it.

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
- **Release-build diagnostics** - the console is gone as of step 3, so
  underrun warnings and stream errors now reach nobody in a release
  build. Left deliberately: the alternative is a small in-app surface
  (a status line under the indicator), which is the better long-term
  answer but is its own UI task. Worth doing if latency trouble ever
  shows up away from a dev build.
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

## Post-v2 review - done

A deliberate read of the whole codebase, cold, before any release -
everything in it had grown by accretion across seven steps and had
never been read as a whole.

What it produced, in the order it landed: the saved loop is now held to
the settings that load it and a lost-capture flag no longer disables
saving for the session; the frame is advanced before it's drawn rather
than part-way through; the state machine left `audio/`; the settings
screen holds an `AppConfig` instead of a copy of one; the pre-roll
became testable; the audio callbacks left their closures, which is what
made the recording alignment measurable at all; recording was landing a
whole monitoring delay ahead of the beat, and the rings were too small
to hold one driver buffer. See the history for the reasoning on each.

Still open:

- **The alignment fix wants confirming by ear.** `MonitorDelay` holds
  the recorded signal back to where the monitored one is, and a test
  measures the remaining offset as zero - but that test can't speak for
  the driver's own input/output offset. Worth stacking four layers
  against a click before calling it settled. The ring-sizing fix in the
  same area wants the same pass: no underruns reported at whatever
  latency you actually run.
- **`looper.rs` still can't be tested.** It owns two `cpal::Stream`s, so
  the rule that a cancelling press beats a pre-roll expiry on the same
  frame is a comment rather than a test - only the half that lives in
  `preroll.rs` is pinned. Its two readouts also call `Instant::now()`
  instead of using the `now` the frame was ticked with, so they can
  disagree with the state beside them by microseconds. Both would fall
  out of separating the streams from the screen state - worth doing if
  that file grows again, not for its own sake.
- **Take layout still exists twice** - incrementally in
  `read_mixed_with_overdub`, in one go in `LoopMirror::layer`, with a
  test asserting they agree. Worth folding into one if a third caller
  ever appears.
- **Per-layer mute** is still unbuilt - see step 2.

### Load-bearing, despite appearances

The cost of reading cold: these all look like dead weight and are not.
Each one is either a bug that has already happened once or a real-time
constraint. Check the reason before touching them - README and the
tests cover every one.

- **`MonitorDelay` in the input path** - looks like a buffer for its
  own sake. The dry path is delayed by `latency_frames` to absorb
  callback jitter, so a recorded signal that isn't delayed with it lands
  that far ahead of the beat it was played against, and every layer
  inherits the error again. Measured by a test; it read 8, 20 and 50
  samples at the matching latency settings before the fix.
- **Ring capacity of `latency_frames + SCRATCH_CAPACITY`** - looks
  over-generous. It was `latency_frames * 2` with half of it prefilled,
  which leaves room for exactly the delay: a driver buffer larger than
  that spilled on every single callback.
- **`loop_mirror::load` taking the whole `AppConfig`** - looks like
  needless coupling for a file read. It's the one place that decides
  whether a saved loop still applies, and both the layer stack and the
  UI thread's copy are seeded from its answer. Splitting the rule lets
  them disagree about what exists.
- **`button_held` in `looper.rs`** - a latch, rather than asking egui
  whether the pointer is on the button. egui reads a cursor drifting off
  the button as a release, which broke long-press-clear from the button
  once already.
- **Mixing before writing in `read_mixed_with_overdub`** - the take is
  mixed into the output *before* the incoming sample is recorded over
  it. Reverse the order and the player hears their own take echoed back
  on top of their live signal a buffer later.
- **64-bit sum, then gain, then clamp** (`mix_at` / `scale_and_clamp`) -
  clamping per layer in 32-bit made a hot four-layer stack permanently
  clipped, with the volume slider unable to rescue it.
- **The recorded window (`start` / `written`) in `Layer`** - not an
  optimisation for its own sake. It's what lets a layer be reused
  without memsetting megabytes inside the audio callback, and the
  alternative is unbounded work in the real-time path.
- **`recorded_len` separate from `loop_len` in `LoopStack`** - one is
  how much has been recorded, the other is where playback wraps.
  Conflating them made the elapsed time read zero for a whole take.
- **`Arming` published to the audio thread** rather than hidden behind
  `Idle`, which would work today. v3's count-in needs the callback to
  know it's counting down.
- **`SCRATCH_CAPACITY` chunking in both callbacks** - bounds one
  callback's work whatever the driver hands us. Overrunning the fixed
  scratch buffers would panic inside the real-time path.

## v3: extended build

Not startable as written - it's a list of components, not an order.
Two things to settle first, at the top of a v3 session:

- **Decide loop / bar-grid sync** (see Open decisions). It gates the
  metronome *and* the drum machine, and if loops become bar-quantized
  the saved loop will want its BPM and bar count stored alongside it.
- **Order the work**, the way v2 was ordered. The dependencies:
  per-layer mute finishes the track abstraction, which the mic track
  needs; one scheduler underpins the metronome, which the drum machine
  is a richer version of; the workspace split is easiest now that the
  review has settled the module boundaries.

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
- **Mic channel** - the second input as a track of its own, with
  independent record-enable, mute and level, once the track abstraction
  is in place. Nothing stands in the way physically: both iD4 inputs
  are on the same stream and the input callback already receives every
  channel interleaved, it just discards all but the chosen one - so no
  second device to open and no cross-device drift. Two things the
  single-channel path never had to face: **per-source monitoring**
  (guitar monitoring is always wanted, live mic monitoring often isn't -
  feedback, and hearing yourself dry is unpleasant), and **levels that
  differ enormously** between an instrument input and a mic preamp, so
  one shared gain isn't enough - eventually an input meter, so it can be
  set by eye. A cheap interim version (two channels summed to mono into
  one loop, no track abstraction needed) was considered for v2 and
  dropped in favour of doing it properly here.
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
