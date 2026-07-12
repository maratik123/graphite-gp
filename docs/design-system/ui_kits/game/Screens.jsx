// Graphite GP UI kit — screens. Uses window.TrackCanvas, window.GP_GEO and the
// design-system components from window.GraphiteGPDesignSystem_1b43c8.
const DS = window.GraphiteGPDesignSystem_1b43c8;
const { Button, IconButton, Badge, Tag, Card, Slider, Switch, SegmentedControl, Stepper, Telemetry, MovePad, CarChip, LapMeter } = DS;
const { CELL } = window.GP_GEO;

const CAR_COLORS = ['var(--car-1)', 'var(--car-2)', 'var(--car-3)', 'var(--car-4)', 'var(--car-5)', 'var(--car-6)'];
const CAR_NAMES = ['You', 'Rival Blue', 'Rival Green', 'Rival Amber', 'Rival Plum', 'Rival Teal'];

// --- precompute a lap path around the centerline ellipse ---
function buildLap(steps) {
  const cx = 360, cy = 240, rx = 240, ry = 132;
  const pts = [];
  for (let i = 0; i <= steps; i++) {
    const th = (Math.PI / 2) - (i / steps) * Math.PI * 2; // start bottom, go clockwise
    pts.push([cx + rx * Math.cos(th), cy + ry * Math.sin(th)]);
  }
  return pts.map((p, i) => {
    const n = pts[Math.min(i + 1, pts.length - 1)];
    return {
      x: p[0], y: p[1],
      vx: Math.round((n[0] - p[0]) / CELL),
      vy: Math.round((n[1] - p[1]) / CELL),
    };
  });
}

// ============================ SETUP ============================
function SetupScreen({ cfg, setCfg, onGenerate }) {
  return (
    <div style={{ maxWidth: 560, margin: '0 auto', padding: '48px 24px' }}>
      <div style={{ textAlign: 'center', marginBottom: 36 }}>
        <div style={{ display: 'inline-flex', alignItems: 'center', gap: 12 }}>
          <span style={{ width: 16, height: 16, borderRadius: '50%', background: 'var(--accent)', border: '2px solid var(--graphite-900)' }} />
          <span style={{ fontFamily: 'var(--font-display)', fontSize: 40, fontWeight: 700, letterSpacing: '-0.02em', color: 'var(--text-ink)' }}>
            GRAPHITE <span style={{ color: 'var(--accent)' }}>GP</span>
          </span>
        </div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--text-muted)', marginTop: 10 }}>
          Grid vector racing
        </div>
      </div>

      <Card eyebrow="New race" title="Set up the grid" grid padding="var(--space-6)">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 24, marginTop: 8 }}>
          <div style={{ display: 'flex', gap: 32, flexWrap: 'wrap' }}>
            <Stepper label="Cars (m)" value={cfg.cars} min={2} max={6} onChange={(v) => setCfg({ ...cfg, cars: v })} />
            <Stepper label="Laps" value={cfg.laps} min={1} max={9} onChange={(v) => setCfg({ ...cfg, laps: v })} />
          </div>
          <div>
            <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-muted)', marginBottom: 8 }}>Difficulty (pilot temperature)</div>
            <SegmentedControl value={cfg.diff} onChange={(v) => setCfg({ ...cfg, diff: v })}
              options={[{ value: 'rookie', label: 'Rookie' }, { value: 'pro', label: 'Pro' }, { value: 'ace', label: 'Ace' }]} />
          </div>
          <Slider label="V_target (design speed)" value={cfg.vtarget} min={3} max={10} step={1}
            format={(v) => `${v} cells/turn`} onChange={(v) => setCfg({ ...cfg, vtarget: v })} />
        </div>
      </Card>

      <div style={{ display: 'flex', gap: 12, marginTop: 24, justifyContent: 'center' }}>
        <Button variant="primary" size="lg" iconLeft={<i data-lucide="shuffle" style={{ width: 18, height: 18 }} />} onClick={onGenerate}>
          Generate track
        </Button>
      </div>
      <div style={{ textAlign: 'center', marginTop: 14, fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text-faint)' }}>
        Procedural · closed loop · valid by construction
      </div>
    </div>
  );
}

// ============================ RACE ============================
function RaceScreen({ cfg, onFinish }) {
  const lap = React.useMemo(() => buildLap(64), []);
  const [i, setI] = React.useState(0);
  const [overlays, setOverlays] = React.useState({ grid: true, heatmap: false, fastest: false });
  const step = 64;

  const cur = lap[i % lap.length];
  const speed = Math.hypot(cur.vx, cur.vy);
  const lapsDone = Math.floor(i / step) % (cfg.laps + 1);
  const trail = React.useMemo(() => {
    const t = [];
    for (let k = 5; k >= 1; k--) t.push([lap[(i - k * 2 + lap.length) % lap.length].x, lap[(i - k * 2 + lap.length) % lap.length].y]);
    return t;
  }, [i, lap]);

  // rivals spaced around the loop
  const cars = [
    { ...cur, color: CAR_COLORS[0], you: true, trail },
    { ...lap[(i + 9) % lap.length], color: CAR_COLORS[1] },
    { ...lap[(i + 18) % lap.length], color: CAR_COLORS[2] },
  ].slice(0, cfg.cars);

  const advance = () => setI((v) => v + 1);
  const legal = ['coast', 'up', 'down', 'left', 'right'];

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 300px', gap: 20, padding: 20, height: '100%', boxSizing: 'border-box' }}>
      {/* left: track */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 14, minWidth: 0 }}>
        {/* HUD strip */}
        <div style={{ display: 'flex', gap: 28, alignItems: 'center', background: 'var(--graphite-900)', padding: '14px 20px', borderRadius: 'var(--radius-2)' }}>
          <div style={{ '--text-muted': '#A69D8C' }}><Telemetry label="SPEED" value={speed.toFixed(2)} tone="accent" size="lg" /></div>
          <div style={{ '--text-ink': '#FBF8F0' }}><Telemetry label="v" value={`(${cur.vx}, ${cur.vy})`} /></div>
          <div style={{ '--text-ink': '#FBF8F0' }}><Telemetry label="POS" value={`(${Math.round(cur.x / CELL)}, ${Math.round(cur.y / CELL)})`} /></div>
          <div style={{ marginLeft: 'auto', minWidth: 130, '--text-muted': '#A69D8C', '--text-ink': '#FBF8F0' }}>
            <LapMeter lap={lapsDone} total={cfg.laps} />
          </div>
        </div>
        {/* toolbar */}
        <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
          <Switch label="Grid" checked={overlays.grid} onChange={(v) => setOverlays({ ...overlays, grid: v })} />
          <Switch label="Heatmap" checked={overlays.heatmap} onChange={(v) => setOverlays({ ...overlays, heatmap: v })} />
          <Switch label="Fastest lap" checked={overlays.fastest} onChange={(v) => setOverlays({ ...overlays, fastest: v })} />
          <div style={{ marginLeft: 'auto' }}>
            <Button variant="ghost" size="sm" onClick={onFinish}>Finish →</Button>
          </div>
        </div>
        <div style={{ border: '1.5px solid var(--graphite-900)', borderRadius: 'var(--radius-2)', overflow: 'hidden', flex: 1, minHeight: 0 }}>
          <TrackCanvas cars={cars} showGrid={overlays.grid} showHeatmap={overlays.heatmap} showFastestLap={overlays.fastest} height="100%" />
        </div>
      </div>

      {/* right: control + standings */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <Card title="Your move" eyebrow="Turn — choose acceleration" padding="var(--space-5)">
          <div style={{ display: 'flex', justifyContent: 'center', margin: '4px 0 14px' }}>
            <MovePad legal={legal} onSelect={advance} size={52} />
          </div>
          <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-muted)', textAlign: 'center', lineHeight: 1.6 }}>
            ±1 per axis · no diagonal accel<br />supercover ⊆ D
          </div>
          <div style={{ marginTop: 12 }}>
            <Button variant="secondary" size="sm" fullWidth iconLeft={<i data-lucide="chevrons-right" style={{ width: 16, height: 16 }} />} onClick={advance}>Coast (·)</Button>
          </div>
        </Card>
        <Card title="Standings" padding="var(--space-5)">
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            {Array.from({ length: cfg.cars }).map((_, k) => (
              <CarChip key={k} color={CAR_COLORS[k]} name={CAR_NAMES[k]} kind={k === 0 ? 'you' : 'ai'} rank={k + 1} active={k === 0} />
            ))}
          </div>
        </Card>
      </div>
    </div>
  );
}

// ============================ LAB ============================
function LabScreen({ cfg, onBack }) {
  const [seed, setSeed] = React.useState(4207);
  const phases = [
    ['Ф1', 'Coarse ring (infield-first)', 'ok'],
    ['Ф2', 'Rasterize to points D', 'ok'],
    ['Ф3', 'Start / finish + grid', 'ok'],
    ['Ф4', 'Static validation', 'ok'],
    ['Ф5', 'Passability oracle', 'ok'],
    ['Ф6', 'Local repair', 'warn'],
    ['Ф7', 'Output artifact', 'ok'],
  ];
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 320px', gap: 20, padding: 20, height: '100%', boxSizing: 'border-box' }}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 14, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ fontFamily: 'var(--font-display)', fontSize: 22, fontWeight: 600, color: 'var(--text-ink)' }}>Track lab</span>
          <Badge tone="ok">VALID</Badge>
          <Tag selected>seed {seed}</Tag>
          <div style={{ marginLeft: 'auto' }}><Button variant="ghost" size="sm" onClick={onBack}>← Menu</Button></div>
        </div>
        <div style={{ border: '1.5px solid var(--graphite-900)', borderRadius: 'var(--radius-2)', overflow: 'hidden', flex: 1, minHeight: 0 }}>
          <TrackCanvas cars={[]} showGrid={true} showHeatmap={true} showFastestLap={true} height="100%" />
        </div>
        <div style={{ display: 'flex', gap: 12 }}>
          <Button variant="primary" iconLeft={<i data-lucide="shuffle" style={{ width: 18, height: 18 }} />} onClick={() => setSeed(Math.floor(1000 + Math.random() * 9000))}>Regenerate</Button>
          <Button variant="secondary" iconLeft={<i data-lucide="play" style={{ width: 18, height: 18 }} />}>Test lap</Button>
        </div>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <Card title="Oracle report" eyebrow="Passability + metrics" padding="var(--space-5)">
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 18 }}>
            <Telemetry label="Vmax" value="7" unit="c/t" />
            <Telemetry label="Tempo" value="0.87" tone="accent" />
            <Telemetry label="Width min" value="3" unit="pts" />
            <Telemetry label="S/F width" value="4" unit="pts" />
          </div>
        </Card>
        <Card title="Generation phases" eyebrow="Ф1 – Ф7" padding="var(--space-5)">
          <div style={{ display: 'flex', flexDirection: 'column', gap: 9 }}>
            {phases.map(([p, name, tone]) => (
              <div key={p} style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, fontWeight: 700, color: 'var(--text-ink)', width: 26 }}>{p}</span>
                <span style={{ fontFamily: 'var(--font-ui)', fontSize: 13, color: 'var(--text-body)', flex: 1 }}>{name}</span>
                <Badge tone={tone}>{tone === 'ok' ? '✓' : 'repair'}</Badge>
              </div>
            ))}
          </div>
        </Card>
      </div>
    </div>
  );
}

// ============================ RESULTS ============================
function ResultsScreen({ cfg, onAgain, onMenu }) {
  return (
    <div style={{ maxWidth: 560, margin: '0 auto', padding: '48px 24px' }}>
      <div style={{ textAlign: 'center', marginBottom: 28 }}>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--text-muted)' }}>Race complete</div>
        <div style={{ fontFamily: 'var(--font-display)', fontSize: 34, fontWeight: 700, letterSpacing: '-0.02em', color: 'var(--text-ink)', marginTop: 6 }}>
          You finished <span style={{ color: 'var(--accent)' }}>P1</span>
        </div>
      </div>
      <Card title="Final standings" grid padding="var(--space-6)">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {Array.from({ length: cfg.cars }).map((_, k) => (
            <div key={k} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <CarChip color={CAR_COLORS[k]} name={CAR_NAMES[k]} kind={k === 0 ? 'you' : 'ai'} rank={k + 1} style={{ flex: 1 }} />
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 13, color: 'var(--text-muted)' }}>{(38 + k * 1.6).toFixed(1)}s</span>
            </div>
          ))}
        </div>
        <div style={{ display: 'flex', gap: 24, marginTop: 20, paddingTop: 16, borderTop: '1px solid var(--border-hairline)' }}>
          <Telemetry label="Fastest lap" value="12.4" unit="s" tone="accent" />
          <Telemetry label="Tempo" value="0.87" />
          <Telemetry label="Crashes" value="1" tone="danger" />
        </div>
      </Card>
      <div style={{ display: 'flex', gap: 12, marginTop: 24, justifyContent: 'center' }}>
        <Button variant="primary" iconLeft={<i data-lucide="rotate-ccw" style={{ width: 18, height: 18 }} />} onClick={onAgain}>Race again</Button>
        <Button variant="secondary" onClick={onMenu}>Menu</Button>
      </div>
    </div>
  );
}

Object.assign(window, { SetupScreen, RaceScreen, LabScreen, ResultsScreen });
