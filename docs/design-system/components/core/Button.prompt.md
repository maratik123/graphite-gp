**Button** — the primary interactive control. Use for any committed action; pick the variant by intent (primary = the main action, secondary = alternative, ghost = low-emphasis/inline, danger = destructive).

```jsx
<Button variant="primary" size="md" onClick={start}>Generate track</Button>
<Button variant="secondary">Cancel</Button>
<Button variant="ghost" size="sm">Reset</Button>
<Button variant="danger">Abandon race</Button>
```

Variants: `primary | secondary | ghost | danger`. Sizes: `sm | md | lg`. Props: `disabled`, `fullWidth`, `iconLeft`, `iconRight`. Icons should be Lucide glyphs sized 16–20 to match the label.
