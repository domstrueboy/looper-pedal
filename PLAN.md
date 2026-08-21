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
- Overdub
- NAM model loading (insert-effect stage)
- Save loop to file
- Tempo sync / metronome
- Waveform rendering

## Documentation update (after v2)

Once any v2 item above is implemented, update both docs (technical +
user) to reflect it — docs stay in sync with whatever's actually built,
not frozen at the MVP.
