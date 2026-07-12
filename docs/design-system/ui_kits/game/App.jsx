// Graphite GP UI kit — app shell + router.
function App() {
  const [screen, setScreen] = React.useState('setup'); // setup | race | lab | results
  const [cfg, setCfg] = React.useState({ cars: 4, laps: 5, diff: 'pro', vtarget: 7 });

  React.useEffect(() => {
    if (window.lucide) window.lucide.createIcons();
  });

  const NavItem = ({ id, icon, label }) => {
    const active = screen === id;
    return (
      <button type="button" onClick={() => setScreen(id)}
        style={{
          display: 'flex', alignItems: 'center', gap: 9, padding: '8px 14px',
          border: 'none', borderRadius: 'var(--radius-2)', cursor: 'pointer',
          background: active ? 'var(--graphite-900)' : 'transparent',
          color: active ? 'var(--paper-0)' : 'var(--text-body)',
          fontFamily: 'var(--font-ui)', fontSize: 'var(--fs-body)', fontWeight: 'var(--fw-medium)',
        }}>
        <i data-lucide={icon} style={{ width: 17, height: 17 }} />{label}
      </button>
    );
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: 'var(--surface-page)' }}>
      {/* top bar */}
      <header style={{ display: 'flex', alignItems: 'center', gap: 16, padding: '10px 18px', borderBottom: '1px solid var(--border-hairline)', background: 'var(--paper-0)', flex: 'none' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
          <span style={{ width: 13, height: 13, borderRadius: '50%', background: 'var(--accent)', border: '2px solid var(--graphite-900)' }} />
          <span style={{ fontFamily: 'var(--font-display)', fontSize: 19, fontWeight: 700, letterSpacing: '-0.02em', color: 'var(--text-ink)' }}>
            GRAPHITE <span style={{ color: 'var(--accent)' }}>GP</span>
          </span>
        </div>
        <nav style={{ display: 'flex', gap: 4, marginLeft: 8 }}>
          <NavItem id="setup" icon="flag" label="New race" />
          <NavItem id="race" icon="gamepad-2" label="Race" />
          <NavItem id="lab" icon="flask-conical" label="Track lab" />
        </nav>
        <div style={{ marginLeft: 'auto', display: 'flex', gap: 8, alignItems: 'center' }}>
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text-muted)' }}>{cfg.cars} cars · {cfg.laps} laps</span>
          <window.GraphiteGPDesignSystem_1b43c8.IconButton label="Settings" variant="ghost"><i data-lucide="settings"></i></window.GraphiteGPDesignSystem_1b43c8.IconButton>
        </div>
      </header>

      {/* body */}
      <main style={{ flex: 1, minHeight: 0, overflow: screen === 'setup' || screen === 'results' ? 'auto' : 'hidden' }}>
        {screen === 'setup' && <SetupScreen cfg={cfg} setCfg={setCfg} onGenerate={() => setScreen('lab')} />}
        {screen === 'lab' && <LabScreen cfg={cfg} onBack={() => setScreen('setup')} />}
        {screen === 'race' && <RaceScreen cfg={cfg} onFinish={() => setScreen('results')} />}
        {screen === 'results' && <ResultsScreen cfg={cfg} onAgain={() => setScreen('race')} onMenu={() => setScreen('setup')} />}
      </main>

      {/* footer cta when in lab */}
      {screen === 'lab' && (
        <div style={{ position: 'fixed', bottom: 22, left: '50%', transform: 'translateX(-50%)' }}>
          <window.GraphiteGPDesignSystem_1b43c8.Button variant="primary" size="lg"
            iconRight={<i data-lucide="arrow-right" style={{ width: 18, height: 18 }} />}
            onClick={() => setScreen('race')}>Start race</window.GraphiteGPDesignSystem_1b43c8.Button>
        </div>
      )}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
