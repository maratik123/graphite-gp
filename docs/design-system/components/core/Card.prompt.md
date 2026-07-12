**Card** — the primary paper surface/panel. Holds content, an optional eyebrow + title header, and a right-aligned header slot (e.g. a Badge). Set `grid` for a faint graph-paper watermark, `selected` for the 2px graphite active border.

```jsx
<Card eyebrow="Track 04 · Procedural" title="Oracle report" right={<Badge tone="ok">VALID</Badge>} grid>
  <Telemetry label="Vmax" value="7" />
</Card>
```

Elevation `0–3` maps to the shadow scale; default 1. Use `onClick` for selectable cards.
