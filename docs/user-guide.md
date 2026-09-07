# Looper Pedal - User Guide

A simple looper pedal for practicing guitar, replacing a hardware looper
like the TC Electronic Ditto. One main control - press to record, press
to loop, press to stop, hold to clear - plus overdub, for stacking a few
layers on top of the loop.

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
- **Orange** (pulsing) - Overdubbing a new layer over the playing loop
- **Amber** - Stopped, loop length still shown, progress bar frozen where
  it left off

## Overdubbing

Once a loop is playing, press **O** (or the **Overdub** button) to start
layering on top of it. Whatever you play is added to the loop; press
**O** again to finish that layer. You can stack up to **4 layers**
including the first recording, and the counter under the buttons shows
where you are.

A few things worth knowing:

- You won't hear the layer you're currently playing come back at you
  while you play it - you're already hearing yourself live. It joins the
  loop from the next time around.
- Keep playing past the end of the loop and it keeps adding to the same
  layer, so you can build something up over a couple of passes without
  erasing what you already put down.
- You don't have to start at the top of the loop. Start wherever you
  like, stop whenever you like - only the part you played is recorded.
- **Remove last** drops the most recent layer if you're not happy with
  it, leaving the others playing. Dropping the only remaining layer
  clears the loop.
- Pressing the main control (space or the round button) while
  overdubbing stops playback, exactly as it does while looping. The
  layer you were recording is kept.

## Changing settings later

Click the small **⚙** icon (top-right of the looper screen) at any time
to reopen Settings and change device, sample rate, or input channel. It
remembers your current choice, so you're not starting from scratch.

## Not yet supported

This is an early, minimal version. Not (yet) included: trimming the
start/end of a loop, muting individual layers, amp modeling (NAM),
saving loops to a file, or a metronome/tempo sync.
