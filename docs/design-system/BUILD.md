# Building the Graphite GP design system locally (Gentoo Linux)

The platform generates `_ds_bundle.js` / `_ds_manifest.json` / `_adherence.oxlintrc.json`
automatically. Those are platform-specific and are **not** produced by this local build —
you don't need them to use the components. This setup rebuilds an equivalent component
bundle (`ds_bundle.js`) with standard tooling.

## 1. Install the toolchain

```bash
sudo emerge -av net-libs/nodejs        # provides node + npm
```

## 2. Install dependencies (run inside this folder)

```bash
npm install
```

This pulls in `esbuild`, `react`, and `react-dom` as declared in `package.json`.

## 3. Build the bundle

```bash
npm run build          # → writes ds_bundle.js
npm run watch          # rebuild on change
```

Output is an IIFE that attaches every component to `window.GraphiteGP`, e.g.
`window.GraphiteGP.Button`, `window.GraphiteGP.MovePad`.

## 4. Use it in an HTML page

```html
<link rel="stylesheet" href="styles.css">   <!-- tokens + @imports; no build needed -->
<script src="https://unpkg.com/react@18.3.1/umd/react.production.min.js"></script>
<script src="https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js"></script>
<script src="ds_bundle.js"></script>
<script>
  const { Button } = window.GraphiteGP;
  ReactDOM.createRoot(document.getElementById('app'))
    .render(React.createElement(Button, { variant: 'primary' }, 'Go'));
</script>
```

(The `--global-name=GraphiteGP` above bundles React in. If you'd rather share the page's
React like the snippet, add `--external:react --external:react-dom` to the build command.)

## 5. Preview

```bash
npm run serve          # static server at http://localhost:3000
# or: python -m http.server
```

## Notes

- `styles.css` already `@import`s the token files and pulls fonts from Google Fonts —
  link it directly, no CSS build step.
- `.d.ts` files are type/metadata hints for the platform editor; esbuild ignores them.
- To add a component, add one `export { X } from './...'` line to `entry.js` and rebuild.
