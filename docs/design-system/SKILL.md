---
name: graphite-gp-design
description: Use this skill to generate well-branded interfaces and assets for Graphite GP — a grid-based vector-racing game (the "Racetrack" pencil game) — either for production or throwaway prototypes/mocks. Contains essential design guidelines, colors, type, fonts, assets, and UI kit components for prototyping.
user-invocable: true
---

Read the `readme.md` file within this skill, and explore the other available files
(`styles.css` + `tokens/`, `guidelines/` foundation cards, `components/`, and the
`ui_kits/game/` recreation).

If creating visual artifacts (slides, mocks, throwaway prototypes, etc), copy assets
out and create static HTML files for the user to view — link `styles.css` for the
tokens, use the graph-paper background motif, Onest + JetBrains Mono, and the
GP-vermilion accent sparingly. If working on production code, copy assets and read
the rules here to become an expert in designing with this brand.

Key facts:
- **Aesthetic:** engineering graph paper marked in graphite, with racing telemetry.
  Warm paper cream ground, warm near-black ink, faint blue grid ruling, one vermilion
  accent, per-car chalk colors, a blue→red speed heatmap.
- **Type:** Onest (display/UI, tight tracking), JetBrains Mono (all
  coordinates, velocity vectors, telemetry). Both substituted from Google Fonts.
- **Icons:** Lucide (CDN, substituted). Native marks are geometric: point/car,
  velocity vector, wall segment, S/F checker, move pad. No emoji.
- **Components** live under `components/` on `window.GraphiteGPDesignSystem_1b43c8`.

If the user invokes this skill without any other guidance, ask them what they want to
build or design, ask some questions, and act as an expert designer who outputs HTML
artifacts _or_ production code, depending on the need.
