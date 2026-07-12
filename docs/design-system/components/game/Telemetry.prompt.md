**Telemetry** — a mono readout of a single labelled metric. Compose several in a row to build the race HUD strip.

```jsx
<Telemetry label="SPEED" value="3.61" tone="accent" size="lg" />
<Telemetry label="v" value="(2, 3)" />
<Telemetry label="LAP" value="2/5" />
<Telemetry label="TEMPO" value="0.87" unit="c/t" tone="muted" />
```

Tones map to the semantic palette. Sizes `sm | md | lg`.
