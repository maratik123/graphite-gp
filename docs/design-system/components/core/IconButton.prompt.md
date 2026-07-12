**IconButton** — a square single-glyph button for toolbar/HUD actions (play, pause, settings, zoom, toggle an overlay). Always pass `label` for accessibility.

```jsx
<IconButton label="Play" onClick={play}><i data-lucide="play"></i></IconButton>
<IconButton label="Grid overlay" active={showGrid} onClick={toggle}><i data-lucide="grid-3x3"></i></IconButton>
<IconButton label="Settings" variant="ghost"><i data-lucide="settings"></i></IconButton>
```

`active` fills graphite for a toggled state. Sizes `sm | md | lg`.
