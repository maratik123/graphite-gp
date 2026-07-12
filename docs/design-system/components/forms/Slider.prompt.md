**Slider** — a range control for continuous parameters (pilot temperature, `V_target`, corridor width). Includes an optional uppercase label and a mono value readout.

```jsx
<Slider label="Pilot temperature" value={t} min={0} max={1} step={0.05}
  format={(v) => `T ${v.toFixed(2)}`} onChange={setT} />
```
