# Algebraic Fractals: user guide

**Algebraic Fractals** draws the complex plane, lighting up the points that are close to an *algebraic number*: a root of a polynomial with whole-number coefficients, such as `3x² − 2x + 1`. The result is star fields, lace, flowers and spirals that continue at every zoom level. You explore them in the main window, save the views you like as **locations**, and turn a group of locations into a smooth **zoom video**.

- [Getting started](#getting-started)
- [The main window](#the-main-window)
- [Which polynomials: order, max coefficient and root density](#which-polynomials-order-max-coefficient-and-root-density)
- [Colours](#colours)
- [Locations and groups](#locations-and-groups)
- [Saving images](#saving-images)
- [Making a video](#making-a-video)
- [What you're looking at](#what-youre-looking-at)
- [Tips](#tips)
- [Troubleshooting](#troubleshooting)
- [Files and folders](#files-and-folders)

---

## Getting started

1. Double-click **Algebraic Fractals.exe** (on a Mac, **Algebraic Fractals.app**: see [MAC.md](MAC.md) the first time). If Windows says "Windows protected your PC", click **More info → Run anyway**: the program isn't signed.
2. The window shows the region around i (from 0.25i to 1.75i, up and down), above the bright points on the real axis, and renders it in a fraction of a second.
3. **Click** anywhere to move the yellow box there, then press **▶ Run** (or Enter) to zoom into it. Or **double-click** to zoom straight in. The **mouse wheel** changes how far each zoom goes.
4. **auto** next to **Max coefficient**, in the strip under the toolbar, is ticked at first: it keeps the star density steady however deep you go.
5. When you find something you like, press **⭐ Save location** (Ctrl+L), give it a name, and file it in a group.
6. Save a few locations in one group, in the order you'd like to visit them, then press **🎬 Make video** on that group in **📂 Locations...**. The video window opens; press **Render now**.

---

## The main window

```
┌────────────────────────────────────────────────────────────────────────────────┐
│ ▶ Run  ⏴ Back  🏠 Home  ➖ Zoom out  🔄 Refresh │ View... │ ⭐ Save location  📂 Locations... │
│ 💾 Save image │ Polynomials...  🎨 Colours... │ 🎬 Video ▾ │ ☰                         │  toolbar
├────────────────────────────────────────────────────────────────────────────────┤
│ Order [3] ☐ auto   Max coefficient [12] ☑ auto   Root density [4] (predicted ~4.1) │
│ Image [square] [512 x 512]      ▓▓▓▓▓░░ 62%  ~2 s left  [Cancel]                   │  settings strip
├────────────────────────────────────────────────────────────────────────────────┤
│                                   the image                                    │
├────────────────────────────────────────────────────────────────────────────────┤
│ X = …   residual …   p(x) = 3x^2 - 2x + 1   length 6  height 3   Rendered in 0.4 s │  status bar
└────────────────────────────────────────────────────────────────────────────────┘
```

Only the settings you change most often are on screen. Everything else is in **popups**, opened by the toolbar's buttons: click a button again, press its ×, or press **Esc** to close the most recent one. Popups float over the image and can be dragged anywhere; several can be open at once, and the image keeps working underneath.

### Mouse and keyboard

| action | effect |
|---|---|
| click | choose the centre of the next zoom (the yellow box moves there) |
| double-click | zoom in there at once |
| mouse wheel | change the zoom factor (how far each zoom goes), from ×1 to ×1000 |
| right-click | **Zoom in here**, **Centre here**, **🌸 Zoom into the flower here**, **🌸 Centre on the flower here**, or **Copy this point** |
| hover | the status bar shows the point, how close it is to an algebraic number (the residual), and the polynomial behind it, with its length (sum of the coefficients' sizes) and height (the largest) |
| **Enter** | zoom into the yellow box |
| **Backspace** | Back |
| **Home** | back to the whole picture |
| **Ctrl+S** | save the image |
| **Ctrl+L** | save the view as a location |
| **Esc** | close the most recently opened popup |

The keys work when no text box is being typed in.

### The toolbar

| button | what it does |
|---|---|
| **▶ Run** | zoom into the yellow box (Enter, or double-click on the image) |
| **⏴ Back** | the previous view (the last 30 are remembered), rendered again |
| **🏠 Home** | back to the home view: centred on i, half-width 0.75 |
| **🌸 Snap to flowers** | when on, a click puts the zoom box on the centre of the largest flower near where you clicked (the simplest quadratic number within the box). The status bar names its polynomial. |
| **➖ Zoom out** | zoom out by the zoom factor (at least ×2), keeping the centre |
| **🔄 Refresh** | render the view again, e.g. after changing the resolution |
| **View...** | the view's exact centre and half-width, which you can type into and **Go** to, or **Copy**; the zoom factor; **Show zoom box** |
| **⭐ Save location** | save this view in a group ([Locations and groups](#locations-and-groups)) |
| **📂 Locations...** | your saved locations, in groups |
| **💾 Save image** | save the image as a PNG ([Saving images](#saving-images)). Right-click it for **Save image as...** |
| **Polynomials...** | which coefficients the polynomials may have, and how much work each pixel takes |
| **🎨 Colours...** | how the image is coloured ([Colours](#colours)) |
| **🎬 Video** | open the video window, start a new video (from a group, or from the whole picture to where you are), or open a saved video project |
| **☰** | **Folders...** (where images and videos are saved), **Show zoom box**, **Reset all settings**, this guide, and the version |

While a video renders, its progress shows at the right of the toolbar; click it to show the video window.

**The View popup.** *Centre* and *Half-width* show the current view exactly (the half-width is the distance from the centre to the edge). Type new values and press **Enter** or **Go** to jump there. The centre accepts forms like `0.5+0.8i`, `-1.2 - 3e-4i`, `2i`, `-i`, `0.3` or `0.5, 0.8`. *Next* shows where the next zoom goes. **🌸 Centre on flower** moves the view's centre exactly onto the centre of the largest flower near it. For a widescreen or window-filling image, the half-width is half the image's *height* (the box says **Half-height**), as in videos. Below a half-width of about 10⁻¹² of the centre's size, 64-bit numbers run out of precision and the picture turns blocky; an orange note warns you.

---

## Which polynomials: order, max coefficient and root density

Each pixel shows how close it is to a root of *any* polynomial `c0 + c1·x + … + cn·xⁿ` allowed by these settings:

| setting | where | meaning |
|---|---|---|
| **Order** | strip | the highest power of `x`. Higher orders reveal many more algebraic numbers (denser, finer patterns) but are slower. |
| **Max coefficient** | strip | the largest coefficient size, e.g. 12 allows −12 to 12. Larger values add more points. Up to 1 000 000, since the sparse sets need large values to have many members. |
| **Root density** | strip | the target for **auto**, in algebraic numbers per 100 square pixels |
| **Image** | strip | the image's shape and size: **square** (128 × 128 to 4096 × 4096), **widescreen 16:9** (640 × 360 to 3840 × 2160), or **fill the window**, which renders at the window's own size and renders again when you resize the window |
| **Coefficients** | Polynomials popup | which sizes are allowed, each with either sign, and 0: **integers**, or the sparser **1 and primes**, **Fibonacci numbers**, **odd numbers**, **triangular numbers**, **squares**, **cubes** and **powers of 2**. Each gives its own patterns. Hover over a set to see its first members. |
| **0 can be a root** | Polynomials popup | count 0 as an algebraic number (the root of the polynomial `x`), so it lights up. Untick it and 0 sits in a dark hole. Polynomials with a spare factor of x, like `x³ − 2x`, are never shown: the simpler `x² − 2` is. |

Changing the order, the max coefficient or a Polynomials setting re-renders the view by itself, once you stop changing it for a moment. Changing the **resolution** doesn't: explore at a low resolution, then raise it and press **Refresh** (or zoom in) when you've found something.

**Root density and auto.** How a star field looks depends mostly on how many algebraic numbers land per pixel. Too few, and it's a scatter of isolated dots; too many, and they merge into noise. The grey **(predicted ~…)** after the root density estimates this for the current view before rendering, usually within about 20%.

Tick **auto** next to **Order** or **Max coefficient** and it's chosen for you, so every view has the **root density** you set (default 4):
- As you zoom in, auto raises the max coefficient or the order to keep the density steady; zooming out lowers it.
- Only one can be automatic at a time. The other settings stay as you set them.
- **auto max coefficient** can match the target closely; **auto order** moves in big steps.
- Auto picks whatever the root density needs, however slow it is to render (up to max coefficient 1 000 000). Watch the progress bar, and **Cancel** or lower the root density if a render is too slow. If even 1 000 000 falls short, an orange **⚠** appears; hover over it to see why.

Some densities to try: 4 gives a fine, even star field. Around 0.1–0.5 gives sparse, distinct stars that show their arrangement. Above 30, the stars blur into texture.

**Render times.** The Polynomials popup shows how much work each pixel takes ("searched in ~N steps"). Render time grows with *steps × pixels*. Measured at 256 × 256 on a 12-core PC (1024 × 1024 takes about 16 times as long):

| settings | steps per pixel | 256 × 256 |
|---|---|---|
| order 3, max 12 | ~300 | instant |
| order 4, max 32 | ~2 000 | under 1 s |
| order 5, max 12 | ~900 | about ½ s |
| order 6, max 8 | ~2 700 | about 1 s |
| order 8, max 4 | ~4 000 | about 1½ s |
| order 8, max 10 | ~100 000 | ½ min away from the real axis, 2 min across it |

**Deep zooms are much faster than the step count suggests.** Once neighbouring pixels share their nearest algebraic numbers, one search serves a whole tile of pixels, with exactly the same picture. At order 3 with max coefficient 3000, a 1280 × 720 view 10⁻⁷ wide takes about 0.03 s instead of 18 s. Views that cross or hug the real axis (the horizontal line through 0) are slower, and don't get tiles. Views centred exactly on it are twice as fast, because the picture is mirror-symmetric.

---

## Colours

**🎨 Colours...** changes the colouring **instantly**, without rendering again. The video window's Colour tab has the same controls.

| Colour by | what you see |
|---|---|
| **1 / residual^n** | brightness = `1 / residual^n`. With **n** = 1, bright points have soft glows. Raise **n** (3–10) to shrink each to a sharp dot. |
| **−log(residual)** | a gentler scale showing the most structure, including faint background texture |
| **length (sum \|ci\|)** | colour by how *complicated* the closest polynomial is: the sum of its coefficients' sizes |
| **height (max \|ci\|)** | the same, using the largest coefficient |

**What and which colours:**
- **Colormap**: classic (bone, gray, hot, copper, pink, spring, summer, autumn, winter, cool, jet, parula), modern scientific (inferno, magma, plasma, viridis, turbo), and rainbow/psychedelic (hsv, sinebow, neon, acid, candy, cosmic). The strip under it shows the colours in use: left the darkest pixels, right the brightest.
- **Invert colormap**: swap dark and light.
- **Shade by 1/res^n** (length/height only): darken away from the algebraic numbers, giving coloured dots on black.
- **Log scale** (length/height only): spread the colormap over the logarithm, so simple polynomials get as much of it as complicated ones.

**Adjust:**

| slider | effect | default |
|---|---|---|
| **n** | sharpness: larger n makes the points sharper | 1 |
| **max %** | the brightness that counts as full: 99.5 saturates the brightest 0.5% of pixels. Lower it for a brighter image. | 99.5 |
| **min %** | the brightness that counts as black: 50 makes the darker half black. Raise it to hide the background haze. | 0 |
| **Smooth stars** | average each pixel's brightness over the pixel instead of taking it at the centre. Stars smaller than a pixel look the same wherever they fall instead of sparkling unevenly, which matters most in videos. | on |
| **brightness** | lighten or darken every colour | 0 |
| **contrast** | spread colours from mid-grey (above 1) or squash them towards it | 1 |
| **gamma** | above 1 brightens the faint points; black and full brightness stay put | 1 |
| **saturation** | 0 = grey, above 1 = more vivid | 1 |
| **colour cycles** | run through the colormap several times, like contour lines: each point becomes a set of rings | 1 |
| **colour shift** | slide the colours along the colormap | 0 |

n, max %, min % and Smooth stars act on the residual, so with length/height colouring they're greyed out when shading is off. For psychedelic images, try **hsv**, **sinebow**, **neon** or **acid** with 5–20 colour cycles: these four wrap around, so the bands join seamlessly. **Reset colours** puts everything back.

---

## Locations and groups

A **location** is a saved view: a name, a centre and a half-width. Nothing else is kept, so going to a location shows it with the settings and colours you have then. Locations are filed in **groups**. A group is usually the start, waypoints and finish of one zoom video, in order.

**Saving.** Press **⭐ Save location** (or Ctrl+L). Type a name (leave it empty to use the centre), choose a group, or **New group...** to start one, and press **Save** (or Enter). It goes at the end of the group. Saving with a name that's already in the group replaces that location's view (the button says **Replace**). New locations go into the group you used last; at first that's **Unsorted**.

**The Locations popup** lists every group:
- Click a location's name to go there. The location you're looking at is highlighted. Hover over a name to see its centre, its half-width, and how much deeper it is than the one before it in the group.
- **⏶ ⏷** move a location up and down its group; **☰** (drag it) moves it anywhere, including onto another group's name. **Move** sends it to another group.
- **✏** renames it, **✖** deletes it (it asks first). Right-click a name for **Set to this view**, which gives the location the view you're looking at.
- On a group's line: **🎬 Make video** opens the video window with the group's locations as the start, waypoints and finish. **⚙** has **Rename**, **Sort by depth** (widest first, so the video zooms in), **Reverse order** and **Delete group...**. Click the triangle to fold a group away.
- **➕ New group** adds an empty group.

**Places from the old explorer.** If you used the old explorer (`algfrac-gui.exe`), its saved places appear in the group **Old places**, with their names and views. Their saved renders aren't used, and the old files are left untouched.

---

## Saving images

**💾 Save image** (Ctrl+S) saves the image at its full resolution, with the current colours, as a PNG in the images folder (☰ → **Folders...**; at first `Pictures\Algebraic Fractals`). Right-click it for **Save image as...** to choose the name and folder. The file name records everything needed to find the image again:

```
alg_o4_m12_+0.50000000+0.80000000i_hw7.500e-1_1024px_pow_inferno_n3.png
     │   │   └ centre                 └ half-width   └ size └ colour by, colormap, n
     │   └ coefficients: max 12
     └ order
```

A letter after the max gives the coefficient set: `p` primes, `f` Fibonacci, `o` odd, `t` triangular, `s` squares, `c` cubes and `b` powers of 2 (none for integers). `_nz` means 0 was not allowed, and `_r0` that 0 could be a root. An existing file is never overwritten: `_2`, `_3`, … is added.

---

## Making a video

A video zooms (and pans) smoothly from a **start**, through any number of **waypoints**, to a **finish**. The usual way to make one:

1. **Explore** and find views you like.
2. **Save each as a location** in one group, in the order the video should visit them. Reorder them in the Locations popup if needed, or use **Sort by depth**.
3. Press **🎬 Make video** on the group (or 🎬 Video → **New video from group**). The video window opens with the locations as the waypoints, and the explorer's settings and colours.
4. **Adjust** in the video window's tabs, watching the **waypoint strip**.
5. Press **Render now**, or **Save** the project and render it later.

🎬 Video → **New video** makes a video from the whole picture to the view you're looking at instead.

### The video window

```
┌ Video: Spiral dive ───────────────────────────────────────────────────────────┐
│ Project [Spiral dive]  [💾 Save] [Save as...] [📂 Open] [New]   ● unsaved      │
│ Path   Colour   Polynomials   Output                                           │
├────────────────────────────────────────────────────────────────────────────────┤
│   the tab                                                                      │
├────────────────────────────────────────────────────────────────────────────────┤
│ Waypoints                                               size: small medium large│
│ [ thumbnail ] 12 s ➡ [ thumbnail ] 8 s ➡ [ thumbnail ]                          │
│ 1 Start          2 arm           3 Finish                                       │
├────────────────────────────────────────────────────────────────────────────────┤
│ 1340 frames · 45 s at 30 fps · 1280 x 720 · ~150 MB · ~25 min    [⏺ Render now] │
└────────────────────────────────────────────────────────────────────────────────┘
```

It's a window of its own: put it beside the main window or on a second screen. Closing it doesn't lose anything (it asks about unsaved changes), and a render carries on while it's closed.

**The waypoint strip** is always there, whatever tab is open: a small render of every waypoint's frame, exactly as the video shows it there, with its number, name, half-width and max coefficient ("auto" if auto chose it; an orange ⚠ if auto fell short). Between them, how long each part of the video takes. They're small so that they render quickly, even for deep views with large max coefficients, and they update as you change things: a search setting re-renders the affected thumbnails a moment after you stop changing it (the old picture stays, dimmed, until then), and a colour change recolours them at once.
- The other waypoints that are inside a thumbnail's frame are marked on it: a cross with the waypoint's number, and the box of its own frame once it's big enough to see (the selected waypoint in yellow). Look at a frame and the Colour tab's example frame are marked the same way.
- **Click** a thumbnail to select that waypoint. **Double-click** to look at it in the main window. **Right-click** for **Show in the explorer**, **Set to the explorer's view**, **Insert halfway after** and **Delete**.
- **size** makes the thumbnails bigger.
- A thumbnail shows the frame shrunk, so its stars look denser than the video's.

### Path tab

The table has a row per waypoint: its number (1 is the **Start**, the last the **Finish**), a name you can change, its **centre** and **half-width** (type into them; a red outline means the text can't be read), the **pan speed** and **keep flower size** setting for the way to the next waypoint, and how many frames that takes. **⏶ ⏷** reorder the waypoints, **✖** deletes one (there are always at least two).

The half-width is half the frame's *height*, the same as the explorer's half-width; widescreen frames show more at the sides.

| button | what it does |
|---|---|
| **➕ Add the explorer's view** | a new waypoint after the selected one, with the view the main window is showing |
| **Set to the explorer's view** | gives the selected waypoint the main window's view |
| **Show in the explorer** | shows the selected waypoint in the main window, to look at it closely, adjust it there and come back with **Set to the explorer's view** |
| **Insert halfway** | adds a waypoint halfway along the segment after the selected one, where the video already passes, so nothing changes until you move it |
| **Centre on flower** | moves the selected waypoint's centre exactly onto the simplest quadratic number within half a frame height: the centre of the largest flower there. The status line names its polynomial. |
| **Reverse** | runs the whole path backwards, turning a zoom in into a zoom out |
| **Replace with a group** | replaces the waypoints with a group's locations |
| **Save as a group** | saves the waypoints as locations in a new group, named after the project |

**Zoom per frame** sets how much each frame magnifies the one before: 1.01 (the default) is slow and smooth, 1.02 a steady glide, 1.05 quick. **Pan speed** is how fast the view may move across the plane, as a percentage of the frame's height per frame: 1% crosses a frame height in 100 frames, about 3 seconds at 30 fps.

**How the path moves:**
- **Zooming:** within each segment, the half-width changes by the same factor every frame, so the zoom looks steady. The centre moves in step with the zoom, so the next waypoint stays put on screen while the view closes in around it, as if flying straight at it.
- **Panning:** the view crosses the plane at a steady speed. A segment between views of the same size is a pure pan.
- **Speed limits:** each segment gets enough frames that neither the zoom per frame nor the pan speed is exceeded; whichever needs more frames sets its length.
- **Stops:** every waypoint is shown exactly, in one frame.

**Look at a frame** (at the bottom of the Path tab) renders any frame of the video exactly as it will appear, at the video's full resolution: the same max coefficient (and fading between two), smoothing and colours. Drag the **frame** slider or press **|<** / **>|**. The grey text gives the frame's centre, half-width, max coefficient and render time. Its render time also calibrates the render-time estimate. Fold it away to stop it rendering.

#### Keeping a flower the same size

Each flower is centred on a quadratic number β, and its dark middle is about constant / M across, where M is the max coefficient. So if M grows by the same factor as the zoom, the flower stays the same size on screen while the video dives into it. Normally auto raises M more slowly, to keep the star density steady, so the dark middle grows until it fills the screen.

1. **Frame the flower** at a waypoint, with the flower in the middle, and press **Centre on flower** to put the centre exactly on β. It must be exactly centred, or it drifts as the zoom deepens.
2. **Add the next waypoint** deeper, at the same centre (type a smaller half-width).
3. **Tick keep flower size** on the first one's row. The next waypoint's max coefficient becomes the first one's times the zoom: 40 at half-width 0.02 gives 640 at 0.00125.

The flower keeps its exact size and shape, but the stars around it get about **4 times denser each time the zoom doubles**. Keep the zoom modest (up to 10–30 times) and start from a sparse view. With reuse, use **fine** coefficient steps for the steadiest flower.

### Colour tab

**One colouring for the whole video**: the same controls as the main window's [Colours](#colours), with an **example frame** of the video beside them, recoloured as you change them. Its slider picks any frame (it's the same frame as the Path tab's Look at a frame). **Copy the explorer's colours** and **Use these colours in the explorer** move colours between the two windows.

**Colour scale for the whole video.** Normally each frame is scaled to its own pixels: its brightness by its own percentiles (max %, min %), and for length or height, the colormap runs from the frame's lowest value to its highest. So every frame is well exposed, but the scale moves a little from frame to frame.
- **Steady brightness** (off by default): the brightness percentiles are measured at the waypoints and change steadily from one to the next, instead of each frame's own, so the brightness doesn't pulse as stars come in. On a ×10 zoom into a flower, it made the overall brightness about 4 times steadier. The waypoints' frames are rendered first, which adds a few frames' time.
- **The length (or height) scale**, when colouring by length or height:
  - **Each frame its own**: the colours of all the stars shift whenever the frame's highest value changes.
  - **Fixed from the waypoints** (the default): one range for the whole video, from the lowest value at any waypoint to the highest, so a given length has the same colour in every frame. The range is shown underneath. Early in a zoom only the bottom of the colormap is used, and higher colours appear as the max coefficient grows. Tick **Log scale** too, or the start's small values get only a sliver of the colormap.
  - **Fixed by hand**: type the range. **Measure** puts the waypoints' range back in. Values outside take the colormap's end colours.

The strip measures a fixed scale from its own thumbnails, so its brightness is a close guide to the video's, not exact.

### Polynomials tab

| setting | meaning |
|---|---|
| **Order**, **Coefficients**, **0 can be a root** | as in the main window, for the whole video |
| **Root density** | per 100 square pixels of the video, the density auto aims for at the waypoints. Auto picks whatever max coefficient that needs, however slow, so check the render time before you start. |
| **Copy the explorer's settings** | the main window's order, coefficients and root density |

**The max coefficient at each waypoint** is set in the table: a number, or **auto** (the default), which chooses it for the root density at that waypoint's view, at the video's resolution. Along a zoom in, auto never gives a waypoint a lower value than the one before. **Auto everywhere** ticks auto at every waypoint.

Between waypoints, every frame takes its max coefficient from a smooth curve through the waypoints' values: between two waypoints it grows by the same factor every frame, which is how the density says it should grow as the view shrinks at a steady rate. It never overshoots, so it can't fall during a zoom in. To shape it, insert a waypoint and set its value.

**Rendering:**
- **Reuse renders between frames** (on): each part of the plane is rendered once, in a *patch* covering several consecutive frames, and each frame is cut out of its patch and averaged down. That's several times faster, and the stars flicker less. The max coefficient then moves in **coefficient steps**: **fine** (13% more polynomials each step), **medium** (28%, the default) or **coarse** (65%). Measured at 1280 × 720 on a 12-core PC:

  | video | every frame separately | with reuse | faster by |
  |---|---|---|---|
  | zoom ×10 into the flower around (1 + i√2)/3, max coefficient 49 → 155, 118 frames | 163 s | 25 s | 6.4× |
  | pan across half a frame height (max coefficient 62), 52 frames | 25 s | 1.8 s | 14× |

  Each patch holds up to 6 frames' worth of pixels, about 24 bytes each at order 3: about 130 MB at 720p, 300 MB at 1080p and 720 MB at 4K, with up to two in memory at a time.
- **Fade in new coefficients** (on): the max coefficient is fractional between waypoints (6.4, say), but only whole coefficients exist. With fading, the frame is rendered at the values either side and blended, so new stars fade in over the frames instead of all appearing at once. It takes up to twice as long.

### Output tab

| control | meaning |
|---|---|
| **File format** | **MP4 video** (H.264, plays everywhere), **PNG frames** (a folder of numbered, lossless images, for editing in other software), or both |
| **Shape** | 16:9 (widescreen) or square |
| **Resolution** | from 640 × 360 up to 3840 × 2160 (4K). Render time grows with the number of pixels. |
| **Smoothing** | renders each frame at 2 × 2 or 3 × 3 times the pixels and averages them down, so tiny stars glide instead of flickering. 4 or 9 times as long. With reuse, frames are already averaged down, so it matters less. |
| **Frame rate** | 24 to 60 frames per second |
| **Quality** | how faithfully the MP4 keeps the fine detail (below). **high** is the default. |
| **Folder**, **File name** | where the video goes (at first, the videos folder and the project's name). An existing file is never overwritten: `_2`, `_3`, … is added. The full path is shown underneath. |

**Quality and file size.** Star fields are about the hardest thing to compress: thousands of tiny points, all moving. So the video is encoded at a constant quality, and the file grows as large as the detail needs. Measured on a 720p zoom:

| quality | picture quality (PSNR) | size at 720p, 30 fps | at 1080p, 30 fps |
|---|---|---|---|
| good | 35 dB: fine detail slightly soft | ~5 MB per second | ~11 MB per second |
| **high** | 41 dB: hard to tell from the frames | ~8 MB per second | ~18 MB per second |
| very high | 44 dB | ~10 MB per second | ~23 MB per second |
| near lossless | 48 dB | ~13 MB per second | ~29 MB per second |

Video sites recompress uploads, so for sharing online **very high** keeps the most detail through their recompression.

### Rendering

The bottom bar shows the number of frames, the length, the size, a rough file size and, once something has been rendered to time it by, a rough render time. **⏺ Render now** saves the project (if it has a name) and renders every frame in order. While it runs:
- the tab area shows the latest frame, the progress, the time left and the size so far. The time left allows for later frames being slower, as the max coefficient rises;
- the settings are locked, but the main window keeps working (both share the processor, so each is slower while the other is busy), and the video window can be closed;
- **⏹ Stop** finishes the current frame and saves the video so far as a shorter video.

When it's done, the Output tab has **Show in folder** and **▶ Play**. Closing the program while a video renders asks whether to stop and save what's done, keep rendering, or quit without saving.

**Render times.** Every frame rendered separately takes about as long as the same image in the explorer (times the smoothing, and up to 2 with fading). Reuse is several times faster. Measured on a 12-core PC, order 3, auto max coefficient, root density 4, every frame separately, without fading:

| video | frames | time |
|---|---|---|
| 1280 × 720, whole view to half-width 0.02 at 1.02 per frame (max coefficient 6 → 43) | 234 (7.8 s at 30 fps) | 1 minute |
| the same view range, with 2 × 2 smoothing | 234 | about 3½ minutes |

Deep frames get much cheaper per pixel (see [Render times](#which-polynomials-order-max-coefficient-and-root-density) in the explorer), so very deep zooms with large max coefficients are practical, as long as they stay away from the real axis (about 2 minutes per 720p frame at max coefficient 3000 there).

### Projects: saving and rendering later

A **video project** is every setting of one video. The project bar at the top:
- **Project**: the name it's saved under.
- **💾 Save** (Ctrl+S) saves it; **Save as...** under a new name. **● unsaved** shows when there are changes since it was last saved or opened.
- **📂 Open** lists the saved projects, **Old paths** (paths saved by the old video maker, brought in as new projects, with their start's colouring for the whole video), and **Import...** / **Export...** to copy a project file to or from anywhere (another computer, a friend).
- **New**: **Blank** (from the whole picture to the explorer's view) or **From group**.

Leaving a project with unsaved changes asks first. If the program closes with unsaved changes, they're kept as the project **(last session)**, which reopens next time.

**Render later** by saving the project, then opening it again from 🎬 Video → **Open video project** and pressing Render now. Or render it from the command line, which leaves the app free (overnight, say):

```
algfrac.exe video "Spiral dive"
```

It prints the progress and the time left; Ctrl+C stops after the current frame and keeps the video so far. `algfrac.exe video --list` lists the saved projects.

---

## What you're looking at

Each star is an algebraic number, and the patterns come from how these numbers repel and organise each other. In brief:

- **Dark holes around simple numbers.** Two different algebraic numbers can't be arbitrarily close. Their *resultant*, a whole number built from both polynomials' coefficients, is a product of their differences and is at least 1. The simpler a number (low degree, small coefficients), the bigger the empty zone around it. That's why 0, ±1, ±i, the roots of unity and so on sit in holes.
- **The dark band along the real axis.** Complex roots come in mirror-image pairs, and two roots of the same polynomial can't be too close (the *discriminant* is a nonzero whole number). So stars avoid the axis, while real roots pile up into the bright line along it.
- **Flowers and spirals.** Near a quadratic number β, the stars are approximately β − λ/μ, where λ and μ lie on a grid of points (a lattice). Dividing by μ turns the grid's straight rows into circles through β: these are the petals. Rotated and scaled copies of the flower can line up into spirals. Every quadratic number has a different lattice, so every flower is different.
- **Why order 3 is special.** At order 2, every complex star has a rational real part, so they sit stiffly on vertical lines. Order 3 is the first order where they're free, but there are few enough cubics that they still gather into flowers around the quadratic numbers. At higher orders, the far greater number of polynomials blurs this into a finer, more even texture, concentrated near the unit circle.
- **Repeating, but never identical.** Transformations like z → 1/z and z → z + 1 turn polynomials into other polynomials of the same order, but with bigger coefficients. So a motif reappears elsewhere distorted and thinner. Only flipping (top/bottom, left/right) and z → 1/z (inside/outside the unit circle) are exact symmetries.
- **Star size.** Each star's glow is roughly proportional to 1/|p′(α)|. Polynomials that cross zero gently give large soft stars, and steep ones give pinpricks.

The [README](../README.md#why-the-patterns-look-the-way-they-do) explains more, with references. For a readable introduction, see John Baez's essay *The Beauty of Roots*, and the paper *Algebraic Number Starscapes* by Harriss, Stange and Trettel.

---

## Tips

- **Start low, go high.** Explore at 256–384 px with a modest order (3–6) and max coefficient (5–15). When something looks good, raise the resolution and press Refresh.
- **The unit circle is where it's busiest.** Roots of polynomials with small coefficients crowd around the circle of radius 1, so zooming near it gives the densest lace. Further out, points thin into isolated stars.
- **Look at the holes.** Simple numbers have dark, empty regions around them, and their edges are often the prettiest spots.
- **Hover to understand what you see.** The status bar names the polynomial behind each bright point.
- **Try length colouring with shading** to see which points are simple and which complicated.
- **Raise n when the image looks foggy.** Sharp dots (n ≈ 4–10) show the arrangement; low n (≈ 0.5–1) shows the overall density.
- **Try the sparser coefficient sets** with auto max coefficient on.
- **Save locations as you go**, into a group per video idea.
- **Try a short, small test video first**: 640 × 360, no smoothing. Then go to 1080p for the real thing.
- **Check the colours deep down.** Select the deepest waypoint and use **Look at a frame**, or **Show in the explorer**.
- **Slow zooms look best.** At 30 fps, 1.01–1.02 per frame takes 15–30 seconds to magnify 10 000 times.
- **Long pans are slow.** A pan across 10 frame heights at 1% takes 1000 frames. Raise that segment's pan speed, or zoom out, pan and zoom back in with waypoints.
- **Colour cycling** with hsv, sinebow, neon or acid gives psychedelic rings that ripple as the video zooms.

---

## Troubleshooting

| problem | fix |
|---|---|
| "Windows protected your PC" | **More info → Run anyway** (the program is unsigned) |
| the window doesn't open in a virtual machine or remote desktop | the program needs OpenGL 2.0 graphics, which some VMs and remote sessions lack. Run it on a normal desktop. |
| a render takes too long | **Cancel**, then lower the order, the max coefficient or the resolution. Views across the real axis are slower, so try zooming to one side of it. |
| the image turns blocky after extreme zooming | the limit of 64-bit precision (half-width below about 10⁻¹² of the centre's size). Zoom out a little. |
| the image is almost all black | lower **max %** or **min %**, lower **n**, raise **gamma**, or switch to **−log(residual)** |
| the image is washed out or hazy | raise **n**, **max %** or **min %**, or **contrast** |
| the video looks blurry or blocky in busy areas | choose a higher **Quality** |
| the stars flicker or crawl in a video | check **Smooth stars** is ticked; then try **Smoothing** |
| the rough render time is far too long | lower the resolution, the smoothing, the frame rate, the order or the root density, or zoom less far. Or untick auto at the deepest waypoints and set a lower max coefficient. |
| the file is too big | lower the **Quality** or the resolution |
| the stars thin out towards the end of a video | auto has hit the largest max coefficient, 1 000 000 (an orange note in the Polynomials tab). Lower the root density. |
| the brightness flickers from frame to frame | tick **Steady brightness** |
| the colours shift during a length/height video | use **Fixed from the waypoints** or **Fixed by hand** |
| new stars pop in suddenly | tick **Fade in new coefficients** |
| the program runs out of memory at 4K | untick **Reuse renders between frames**, or use a lower resolution |
| the density drifts between two far-apart waypoints | insert a waypoint between them: auto sets its max coefficient, and the curve follows it |

---

## Files and folders

- **Images** go to `Pictures\Algebraic Fractals` (on a Mac `~/Pictures/Algebraic Fractals`) and **videos** to `Videos\Algebraic Fractals` (`~/Movies/Algebraic Fractals`), unless you choose other folders in ☰ → **Folders...**. Each video project keeps its own folder.
- **Your locations, video projects and settings** live in `%APPDATA%\AlgebraicFractals\` on Windows (`~/Library/Application Support/AlgebraicFractals/` on a Mac): `locations.tsv`, `projects\*.afvideo` and `settings.tsv`. They're plain text, so you can back them up or copy them to another computer. ☰ → **Reset all settings** puts the settings back to their defaults without touching locations or projects.
- **The command line**, `algfrac.exe`, renders single images (and the raw data, as `.algf`) for scripts or very large renders: run `algfrac.exe --help`. It also renders video projects (above). [DEVELOPER.md](DEVELOPER.md) describes it, the search, and how to build the program.
