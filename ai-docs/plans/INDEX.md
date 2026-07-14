# Plan index

Active and completed task plans (spec + design pairs). Statuses: ✅ implemented · 🟢 in progress · 🟡 spec-only / deferred · 🔴 blocked.

Written by `/task` (Step 12 flips a row to ✅) and read by `/next`. New plans land at `ai-docs/plans/YYYY-MM-DD-name.{spec,design}.md`; completed pairs move to `ai-docs/plans/done/`.

| Date | Name | Status | Depends on | Notes |
|------|------|--------|------------|-------|
| 2026-07-15 | core-geom-corridor | ✅ implemented (20 new tests; 32 gp-core green) | core-supercover | gp-core corridor-graph helpers (block 3a, build-order 2/40) — 4-conn `flood_fill`/`component_count`, `bounded_complement_components` (§2 Ф4 infield-hole test), in-`D` geodesic BFS (`CorridorScratch`/`geodesic_bfs`/`geodesic_layers`), `walls_from_boundary` (Ф7). `Wall` → `{ cell, side: Side }`; `geom.rs` split into `geom/{mod,graph}.rs`. Closes #5. |
| 2026-07-14 | import-ci-workflows | ✅ implemented (0 new tests; 12 gp-core green) | — | Bootstrap GitHub Actions CI from quartzite (single `ubuntu-latest` lane) + Dependabot + mandatory Vulkan env-init + MSRV 1.97.0 + `CARGO_BUILD_WARNINGS=deny` + workspace lint tables (pedantic/nursery=deny). No tracking issue. |
| 2026-07-13 | core-supercover | ✅ implemented (12 tests) | — | gp-core exact integer `supercover` predicate (SAT bbox-scan); foundation of `legal_move` + passability oracle. Closes #4. |
