# Looper Pedal - User Guide

A simple looper pedal for practicing guitar, replacing a hardware looper
like the TC Electronic Ditto. One control, one track, no overdub - press
to record, press to loop, press to stop, hold to clear.

## First run

The first time you launch the app, you'll see a **Settings** screen:

1. **ASIO device** - pick your audio interface
2. **Sample rate** - pick a rate your interface supports (44100 Hz is a
   safe default)
3. **Input channel** - pick whichever input your guitar cable is
   actually plugged into (e.g. "Input 1"). Only this one channel is used;
   it's centered equally in both ears when you monitor, so it doesn't
   matter that it came from a single input.

Click **Start**. Your choices are remembered, so next time the app opens
straight into the looper - you won't see Settings again unless you
reopen it yourself (see below) or your saved device becomes unavailable
(e.g. the interface is unplugged).

## Using the looper

One control drives everything - either the **spacebar** or the on-screen
round button. They're fully interchangeable.

| Press | What happens |
|---|---|
| 1st press | Start recording |
| 2nd press | Stop recording, loop starts playing immediately |
| 3rd press | Stop the loop (silence, but it's still remembered) |
| 4th press | Resume playing the same loop |
| **Hold for ~2 seconds** | Clear the loop, from any state - back to empty |

Your live guitar signal is always audible, whether or not a loop is
playing - the loop just plays back on top of it.

The colored dot shows what's happening:

- **Gray** - Empty, nothing recorded
- **Red** (pulsing) - Recording, with elapsed time shown
- **Green** - Looping, with the loop's length and a progress bar showing
  where in the loop you currently are
- **Amber** - Stopped, loop length still shown, progress bar frozen where
  it left off

## Changing settings later

Click the small **⚙** icon (top-right of the looper screen) at any time
to reopen Settings and change device, sample rate, or input channel. It
remembers your current choice, so you're not starting from scratch.

## Not yet supported

This is an early, minimal version. Not (yet) included: trimming the
start/end of a loop, undo, overdubbing multiple layers, amp modeling
(NAM), saving loops to a file, or a metronome/tempo sync.
