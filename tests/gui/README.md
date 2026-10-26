# GUI test harness

Runs Annotshot on an invisible X11 screen (Xvfb) and drives it with simulated
mouse and keyboard input, taking screenshots to look at. Your desktop session
(Wayland or not) is not touched: the app runs on its own screen using GTK's
X11 backend.

It covers drawing, selecting, text editing, dialogs and saving. It does not
cover Wayland/GNOME specifics (clipboard handover after closing, the portal
save dialog, the launcher), and screenshots don't show the mouse cursor.

## Setup

```
sudo apt install xvfb xdotool xclip x11-apps python3-gi
cargo build
```

## Use

```
tests/gui/smoke.sh                 # example run, screenshots in tests/gui/out/
```

Or step by step:

```
tests/gui/gui.sh start
python3 tests/gui/mkshot.py tests/gui/out/sample.png 700 400
tests/gui/gui.sh paste tests/gui/out/sample.png
tests/gui/gui.sh key t
tests/gui/gui.sh click 500 450
tests/gui/gui.sh type "Hello"
tests/gui/gui.sh shot hello            # → tests/gui/out/hello.png
tests/gui/gui.sh stop
```

Run `tests/gui/gui.sh` without arguments for all commands. The window sits at
0,0 (1100×750) on a 1280×800 screen, so screen coordinates equal window
coordinates. `ANNOTSHOT_BIN` and `ANNOTSHOT_TEST_DISPLAY` (default `:99`)
override the app binary and display.

## Demo GIF

`tests/gui/demo.sh` records the README's `docs/demo.gif` (needs `ffmpeg`): it
pastes a made-up settings page from `mkdemo.py`, marks up a bug report and
then adjusts the annotations. Run it after UI changes that show in the demo;
the script drives the mouse by image coordinates, so changing the page layout
in `mkdemo.py` means updating the coordinates too.
