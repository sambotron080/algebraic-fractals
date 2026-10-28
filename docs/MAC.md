# Algebraic Fractals on a Mac

The Mac version works the same as the Windows one: see [USER-GUIDE.md](USER-GUIDE.md). It runs on macOS 11 (Big Sur) or later, on both Apple silicon and Intel Macs.

| file | what it is |
|---|---|
| **Algebraic Fractals.app** | the app: the explorer and the video window |
| `algfrac` | the command-line renderer (run it from Terminal) |

Drag the app into **Applications**, or run it from anywhere.

## The first time you open it

The app isn't signed with an Apple developer certificate, so macOS blocks it the first time with a message like *"Apple could not verify 'Algebraic Fractals' is free of malware"*. To allow it:
1. Click **Done** (or **OK**).
2. Open **System Settings → Privacy & Security** and scroll down to the message about the app.
3. Click **Open Anyway**, and confirm with your password.

You only need to do this once. On macOS 14 and earlier, you can instead right-click the app, choose **Open**, then click **Open** in the dialog.

If macOS says the app **"is damaged and can't be opened"**, it was blocked when it was downloaded. Clear the download flag in Terminal, then open it as above:

```
xattr -dr com.apple.quarantine "/Applications/Algebraic Fractals.app"
```

## Where files go

- **Locations, video projects and settings** are kept in `~/Library/Application Support/AlgebraicFractals/`.
- **Images** go to `~/Pictures/Algebraic Fractals` and **videos** to `~/Movies/Algebraic Fractals`, unless you choose other folders in ☰ → **Folders...**.
