# Graphite GP — Game UI Kit

An interactive, high-fidelity recreation of the Graphite GP game surfaces,
composed from this design system's components. **Because the source project is
scaffold-only (no rendering backend chosen, no UI code), this kit is an
interpretation of the design doc §4 render spec, not a recreation of shipped
screens.**

## Screens / flow

`index.html` is a click-through of the whole loop:

1. **New race** (`SetupScreen`) — pick cars `m`, laps, difficulty (pilot
   temperature), and `V_target`. → Generate track.
2. **Track lab** (`LabScreen`) — the generated closed track on graph paper with
   heatmap + fastest-lap overlays, the oracle report (Vmax, tempo, widths), and
   the Ф1–Ф7 generation phases. → Start race.
3. **Race** (`RaceScreen`) — the hero: the graph-paper track canvas, the
   telemetry HUD, the **MovePad** (click a move / Coast to advance the car along
   the lap), overlay toggles, and live standings.
4. **Results** (`ResultsScreen`) — final standings, fastest lap, crashes.

## Files

- `index.html` — shell: loads React + Babel + the DS bundle + Lucide, mounts the app.
- `Track.jsx` — `TrackCanvas`, the SVG track (regions / walls / S-F / overlays /
  cars-as-points with velocity vectors + trails). Exports `window.TrackCanvas`,
  `window.GP_GEO`.
- `Screens.jsx` — `SetupScreen`, `RaceScreen`, `LabScreen`, `ResultsScreen`.
- `App.jsx` — top bar, nav, screen router.

## Components used

Button, IconButton, Badge, Tag, Card (core); Slider, Switch, SegmentedControl,
Stepper (forms); Telemetry, MovePad, CarChip, LapMeter (game).

## Notes / fidelity

- The track is drawn as an idealized closed annulus (rounded-rect corridor with a
  single infield hole) — enough to read as a valid Racetrack loop; real tracks
  come from the Ф1–Ф7 generator.
- Car motion samples a centerline ellipse; the MovePad advances the player one
  step. Physics (supercover legality, crash, collisions) is faked for the mock.
- Icons are Lucide (substituted — see root readme).
