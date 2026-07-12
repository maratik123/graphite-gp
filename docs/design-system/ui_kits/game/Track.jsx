// Graphite GP — Track canvas (SVG). Draws the three regions (outfield /
// infield / asphalt), walls on the half-grid, the S/F line, optional analytics
// overlays (speed heatmap, fastest-lap line, graph-paper grid), and the cars as
// points with velocity vectors + fading trails. This is game content, drawn as
// vector geometry on the lattice — per design doc §4.
const { useMemo } = React;

// rounded-rect path
function rr(x, y, w, h, r) {
  return `M${x + r},${y} h${w - 2 * r} a${r},${r} 0 0 1 ${r},${r} v${h - 2 * r} a${r},${r} 0 0 1 ${-r},${r} h${-(w - 2 * r)} a${r},${r} 0 0 1 ${-r},${-r} v${-(h - 2 * r)} a${r},${r} 0 0 1 ${r},${-r} z`;
}

const W = 720, H = 480, CELL = 24;
// outer + inner boundary of the corridor D
const OUT = { x: 48, y: 48, w: 624, h: 384, r: 84 };
const INN = { x: 192, y: 168, w: 336, h: 144, r: 44 };
// racing centerline (render-only), between the two boundaries
const MID = { x: 120, y: 108, w: 480, h: 264, r: 64 };

function TrackCanvas({
  cars = [],
  showGrid = true,
  showHeatmap = false,
  showFastestLap = false,
  height = 'auto',
}) {
  const gridId = useMemo(() => 'gpg' + Math.random().toString(36).slice(2, 7), []);
  const heatId = useMemo(() => 'heat' + Math.random().toString(36).slice(2, 7), []);

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={height}
      style={{ display: 'block', background: 'var(--paper-1)', borderRadius: 'var(--radius-2)' }}
      preserveAspectRatio="xMidYMid meet">
      <defs>
        <pattern id={gridId} width={CELL} height={CELL} patternUnits="userSpaceOnUse">
          <path d={`M${CELL} 0 L0 0 0 ${CELL}`} fill="none" stroke="var(--grid-line)" strokeWidth="1" />
        </pattern>
        <pattern id={gridId + 'L'} width={CELL} height={CELL} patternUnits="userSpaceOnUse">
          <path d={`M${CELL} 0 L0 0 0 ${CELL}`} fill="none" stroke="rgba(251,248,240,0.3)" strokeWidth="1" />
        </pattern>
        <clipPath id={gridId + 'C'}>
          <path d={`${rr(OUT.x, OUT.y, OUT.w, OUT.h, OUT.r)} ${rr(INN.x, INN.y, INN.w, INN.h, INN.r)}`} clipRule="evenodd" />
        </clipPath>
        <linearGradient id={heatId} x1="0" y1="1" x2="1" y2="0">
          <stop offset="0" stopColor="var(--heat-0)" />
          <stop offset="0.4" stopColor="var(--heat-1)" />
          <stop offset="0.7" stopColor="var(--heat-2)" />
          <stop offset="1" stopColor="var(--heat-3)" />
        </linearGradient>
      </defs>

      {/* 1a. outfield */}
      <rect x="0" y="0" width={W} height={H} fill="var(--paper-1)" />
      {showGrid && <rect x="0" y="0" width={W} height={H} fill={`url(#${gridId})`} />}

      {/* 1b. asphalt = corridor D (even-odd ring), tinted by heatmap when on */}
      <path d={`${rr(OUT.x, OUT.y, OUT.w, OUT.h, OUT.r)} ${rr(INN.x, INN.y, INN.w, INN.h, INN.r)}`}
        fillRule="evenodd" fill={showHeatmap ? `url(#${heatId})` : 'var(--asphalt-1)'} opacity={showHeatmap ? 0.9 : 1} />

      {/* 1b-grid. graph-paper showing through the asphalt (light, clipped to corridor) */}
      {showGrid && (
        <rect x="0" y="0" width={W} height={H} fill={`url(#${gridId}L)`} clipPath={`url(#${gridId}C)`} />
      )}

      {/* 1c. infield (the hole) — distinct so the loop reads */}
      <path d={rr(INN.x, INN.y, INN.w, INN.h, INN.r)} fill="var(--surface-infield)" />
      {showGrid && <path d={rr(INN.x, INN.y, INN.w, INN.h, INN.r)} fill={`url(#${gridId})`} />}

      {/* 2. walls = fill boundary (never through a point) */}
      <path d={rr(OUT.x, OUT.y, OUT.w, OUT.h, OUT.r)} fill="none" stroke="var(--wall)" strokeWidth="3" />
      <path d={rr(INN.x, INN.y, INN.w, INN.h, INN.r)} fill="none" stroke="var(--wall)" strokeWidth="3" />

      {/* 4. fastest-lap / centerline overlay */}
      {showFastestLap && (
        <path d={rr(MID.x, MID.y, MID.w, MID.h, MID.r)} fill="none"
          stroke="var(--accent)" strokeWidth="2" strokeDasharray="2 6" strokeLinecap="round" opacity="0.9" />
      )}

      {/* 3. S/F line — checkered segment across the bottom straight */}
      <g>
        {Array.from({ length: 5 }).map((_, i) => (
          <rect key={i} x={352} y={312 + i * 24} width={16} height={24}
            fill={i % 2 === 0 ? 'var(--graphite-900)' : 'var(--paper-0)'}
            stroke="var(--graphite-900)" strokeWidth="1" />
        ))}
      </g>

      {/* 6. cars — points with fading trails + velocity vectors */}
      {cars.map((c, ci) => (
        <g key={ci}>
          {(c.trail || []).map((p, i) => (
            <circle key={i} cx={p[0]} cy={p[1]} r="4"
              fill={c.color} opacity={0.12 + 0.12 * i} />
          ))}
          {c.vx !== undefined && (c.vx !== 0 || c.vy !== 0) && (
            <g stroke="var(--graphite-900)" strokeWidth="2.5" fill="var(--graphite-900)">
              <line x1={c.x} y1={c.y} x2={c.x + c.vx * CELL} y2={c.y + c.vy * CELL} />
            </g>
          )}
          <circle cx={c.x} cy={c.y} r="8" fill={c.color} stroke="var(--graphite-900)" strokeWidth="2.5" />
          {c.you && <circle cx={c.x} cy={c.y} r="13" fill="none" stroke="var(--accent)" strokeWidth="1.5" strokeDasharray="2 3" />}
        </g>
      ))}
    </svg>
  );
}

window.TrackCanvas = TrackCanvas;
window.GP_GEO = { W, H, CELL, OUT, INN, MID };
