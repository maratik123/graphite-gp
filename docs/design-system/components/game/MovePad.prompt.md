**MovePad** — the signature control: the 5 von Neumann accelerations as a plus-shaped keypad (coast `·` center, `↑ ↓ ← →` around). Each cell is an acceleration `(a, b)`. Pass `legal` to mask illegal moves; `value` marks the chosen one.

```jsx
<MovePad
  value={move}
  legal={['coast', 'up', 'right']}   // supercover-legal this turn
  onSelect={(key, {a, b}) => applyAccel(a, b)}
/>
```

Diagonal acceleration is impossible by design — only these 5 exist. `legal={null}` enables all.
