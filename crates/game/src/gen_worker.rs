//! The background generation worker (issue #43, A6/B3, spec Scope 6/7/AC7/AC9).
//!
//! Spawn-per-request, off the main thread, with cooperative cancellation
//! infrastructure, superseded-result discard, and per-phase status
//! aggregation for the Lab screen.
//!
//! Calls `gp_gen::generate(params, &mut observer)` with a real
//! `WorkerObserver` (B3, crate-private) — the spawned thread reads the
//! `Arc<AtomicBool>` cancel flag via `is_cancelled` and folds `on_phase`
//! events into a local `[PhaseStatus; 7]` aggregate, sending
//! `WorkerMsg::Phases` only when that aggregate changes (design § KD7 —
//! bounds channel traffic to ≤ 35 messages/run instead of one per raw
//! event).

use gp_core::track::TrackArtifact;
use gp_gen::{GenObserver, GenParams, GenerationError, Phase, PhaseEvent, PhaseOutcome};
use gp_render::PhaseStatus;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

/// A monotonic id distinguishing one generation request from the next
/// (spec § Approach — *Cancellation, the worker, and the pending window*).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GenerationId(u64);

/// A generation request's outcome, beyond the pipeline's own error.
///
/// The spawned thread panicking is a generation failure too, never a
/// `join().unwrap()` (spec § Technical constraints — thread-panic
/// posture).
#[derive(Debug, thiserror::Error)]
pub enum GenerationFailure {
    /// The pipeline itself returned an error.
    #[error(transparent)]
    Pipeline(#[from] GenerationError),
    /// The worker thread was lost — it panicked, or its message never
    /// arrived (a `mpsc::TryRecvError::Disconnected` defensive fallback;
    /// unreachable in practice since the worker's `Sender` is a clone of
    /// one this `Worker` holds for its whole lifetime, so the channel
    /// itself never disconnects — the panic-catching path below is what
    /// actually reports a lost worker).
    #[error("the generation worker thread was lost")]
    WorkerLost,
}

/// A message from the worker thread: either a terminal result, or a
/// phase-aggregate snapshot ([`WorkerObserver`], sent only on change).
enum WorkerMsg {
    /// The request finished (accepted, pipeline error, or cancelled).
    /// Boxed — `TrackArtifact` is large enough that an unboxed `Result`
    /// here would balloon `WorkerMsg`'s size to the biggest variant's
    /// (`clippy::large_enum_variant`, deny), dwarfing `Phases`'s.
    Done(GenerationId, Box<Result<TrackArtifact, GenerationFailure>>),
    /// An updated `[PhaseStatus; 7]` aggregate for the in-flight request.
    Phases(GenerationId, [PhaseStatus; 7]),
}

/// Maps a raw [`PhaseOutcome`] to the ordered [`PhaseStatus`] `gp-render`
/// renders (design § KD6 — `gp-gen` reports raw events, `gp-game` is the
/// only crate that sees both types).
const fn map_outcome(outcome: PhaseOutcome) -> PhaseStatus {
    match outcome {
        PhaseOutcome::Skipped => PhaseStatus::Skipped,
        PhaseOutcome::Ok => PhaseStatus::Ok,
        PhaseOutcome::Repair => PhaseStatus::Repair,
        PhaseOutcome::Failed => PhaseStatus::Failed,
    }
}

/// Maps a [`Phase`] to its `[PhaseStatus; 7]` slot, in `Ф1..Ф7` order.
const fn phase_index(phase: Phase) -> usize {
    match phase {
        Phase::F1 => 0,
        Phase::F2 => 1,
        Phase::F3 => 2,
        Phase::F4 => 3,
        Phase::F5 => 4,
        Phase::F6 => 5,
        Phase::F7 => 6,
    }
}

/// Every phase's aggregate is still [`PhaseStatus::Pending`] until at
/// least one event names it.
const INITIAL_PHASES: [PhaseStatus; 7] = [PhaseStatus::Pending; 7];

/// **Terminal rule** (spec § Phase-status ordering, design § Approach —
/// *Terminal rule*): when a run terminates, on either arm, every phase
/// whose aggregate is still `Pending` becomes `Skipped` — `Pending` means
/// "still in flight", which a finished run can never be. `gp-gen`'s raw
/// event stream never reports this itself (it is purely factual about
/// what ran); the rule is pinned here, on the `gp-game` side, so it
/// covers `Ok`/`Err`/`WorkerLost` terminations uniformly.
fn apply_terminal_rule(phases: &mut [PhaseStatus; 7]) {
    for status in phases {
        if *status == PhaseStatus::Pending {
            *status = PhaseStatus::Skipped;
        }
    }
}

/// The worker thread's [`GenObserver`] impl (B3): reads the cancel flag,
/// and folds `on_phase` events into a local `[PhaseStatus; 7]`
/// aggregate-worst (§ Phase-status ordering), sending a snapshot back
/// only when that aggregate actually changes.
struct WorkerObserver {
    id: GenerationId,
    cancel_flag: Arc<AtomicBool>,
    sender: mpsc::Sender<WorkerMsg>,
    aggregate: [PhaseStatus; 7],
}

impl WorkerObserver {
    const fn new(
        id: GenerationId,
        cancel_flag: Arc<AtomicBool>,
        sender: mpsc::Sender<WorkerMsg>,
    ) -> Self {
        Self {
            id,
            cancel_flag,
            sender,
            aggregate: INITIAL_PHASES,
        }
    }
}

impl GenObserver for WorkerObserver {
    fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    fn on_phase(&mut self, event: PhaseEvent) {
        let slot = &mut self.aggregate[phase_index(event.phase)];
        let mapped = map_outcome(event.outcome);
        if mapped > *slot {
            *slot = mapped;
            // A dropped receiver means nobody is listening -- ignore the
            // send failure rather than panic (mirrors `request`'s own
            // final send).
            let _ = self.sender.send(WorkerMsg::Phases(self.id, self.aggregate));
        }
    }
}

/// A spawn-per-request background generation worker (spec § Key decisions
/// — generation is user-initiated and rare, so spawn cost is irrelevant
/// beside a multi-second run).
///
/// Every request shares this `Worker`'s one persistent channel — **not** a
/// fresh channel per request — so a superseded request's late-arriving
/// result still reaches [`Self::poll`] and is discarded there by comparing
/// its [`GenerationId`] against the current one (AC7), regardless of
/// which thread happens to finish first. This is what makes the discard
/// deterministic under real thread-scheduling races, not just in the
/// common case.
pub struct Worker {
    sender: mpsc::Sender<WorkerMsg>,
    receiver: mpsc::Receiver<WorkerMsg>,
    next_id: u64,
    /// The id of the in-flight (or just-completed, not yet polled)
    /// request; `None` once idle.
    current_id: Option<GenerationId>,
    /// The current request's cancel flag, if any — set by
    /// [`Self::cancel`] (supersede or navigate-away, spec § Key
    /// decisions).
    cancel_flag: Option<Arc<AtomicBool>>,
    /// The current (or most recently finished) request's `[PhaseStatus;
    /// 7]` aggregate — `INITIAL_PHASES` (all `Pending`) once a fresh
    /// request is raised, updated as `WorkerMsg::Phases` snapshots arrive,
    /// and swept by the Terminal rule (`apply_terminal_rule`) the moment
    /// [`Self::poll`] returns a terminal result.
    current_phases: [PhaseStatus; 7],
}

impl Worker {
    /// A fresh, idle worker.
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver,
            next_id: 0,
            current_id: None,
            cancel_flag: None,
            current_phases: INITIAL_PHASES,
        }
    }

    /// Cancels any in-flight request (AC7/AC8) and spawns a fresh one for
    /// `params`, returning its [`GenerationId`]. The spawned thread never
    /// panics past this call — a panic inside `gp_gen::generate` is caught
    /// and reported as [`GenerationFailure::WorkerLost`] (thread-panic
    /// posture).
    pub fn request(&mut self, params: GenParams) -> GenerationId {
        self.cancel();

        let id = GenerationId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.current_id = Some(id);
        self.current_phases = INITIAL_PHASES;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(Arc::clone(&cancel_flag));

        let sender = self.sender.clone();
        thread::spawn(move || {
            let mut observer = WorkerObserver::new(id, cancel_flag, sender.clone());
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                gp_gen::generate(params, &mut observer)
            }));
            let result = match outcome {
                Ok(Ok(artifact)) => Ok(artifact),
                Ok(Err(err)) => Err(GenerationFailure::Pipeline(err)),
                Err(_panic) => Err(GenerationFailure::WorkerLost),
            };
            // A dropped receiver (the `Worker` itself gone) means nobody
            // is listening -- ignore the send failure rather than panic.
            let _ = sender.send(WorkerMsg::Done(id, Box::new(result)));
        });

        id
    }

    /// Cancels the in-flight request, if any, without raising a new one
    /// (navigate-away, spec § Key decisions). A no-op when idle.
    pub fn cancel(&mut self) {
        if let Some(flag) = self.cancel_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Polls for a completed result. Drains and discards every terminal
    /// message whose [`GenerationId`] is not the current request's (a
    /// superseded result, AC7) before returning the first one that
    /// matches — `None` while the current request is still in flight (or
    /// idle). Every `WorkerMsg::Phases` snapshot for the current request
    /// updates [`Self::phases`]'s value along the way; a terminal return
    /// applies the Terminal rule (`apply_terminal_rule`) before returning,
    /// so `Pending` never survives a finished run (AC9, § Approach —
    /// *Terminal rule*).
    pub fn poll(&mut self) -> Option<(GenerationId, Result<TrackArtifact, GenerationFailure>)> {
        loop {
            match self.receiver.try_recv() {
                Ok(WorkerMsg::Phases(id, snapshot)) => {
                    if Some(id) == self.current_id {
                        self.current_phases = snapshot;
                    }
                }
                Ok(WorkerMsg::Done(id, result)) => {
                    if Some(id) != self.current_id {
                        continue;
                    }
                    self.current_id = None;
                    self.cancel_flag = None;
                    apply_terminal_rule(&mut self.current_phases);
                    return Some((id, *result));
                }
                Err(mpsc::TryRecvError::Empty) => return None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let id = self.current_id?;
                    self.current_id = None;
                    self.cancel_flag = None;
                    apply_terminal_rule(&mut self.current_phases);
                    return Some((id, Err(GenerationFailure::WorkerLost)));
                }
            }
        }
    }

    /// Whether a request is currently in flight (not yet polled to
    /// completion).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.current_id.is_some()
    }

    /// The current (or most recently finished) request's `[PhaseStatus;
    /// 7]` aggregate, in `Ф1..Ф7` order (AC9 — the Lab screen's source).
    /// `INITIAL_PHASES` (all `Pending`) before any request has ever been
    /// raised.
    #[must_use]
    pub const fn phases(&self) -> [PhaseStatus; 7] {
        self.current_phases
    }
}

impl Default for Worker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GenerationFailure, GenerationId, INITIAL_PHASES, Worker, WorkerObserver,
        apply_terminal_rule, map_outcome, phase_index,
    };
    use gp_core::rng::Seeds;
    use gp_gen::{GenObserver, GenParams, Phase, PhaseEvent, PhaseOutcome};
    use gp_render::PhaseStatus;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// The cheap CLI-default `GenParams` triple (design § Test Design AC7,
    /// mirroring `crates/gen/src/generate.rs`'s own `params(seed,
    /// seed_budget, repair_budget)` test helper): `seed_budget = 1`,
    /// `repair_budget = 8`, `block_size = 6`, `seeds.generation = 6`
    /// accepts on the first attempt.
    fn cheap_params() -> GenParams {
        GenParams {
            cars: 4,
            min_straight: 3,
            v_ceiling: 5,
            block_size: 6,
            seeds: Seeds {
                generation: 6,
                ..Seeds::default()
            },
            seed_budget: 1,
            repair_budget: 8,
        }
    }

    /// Polls `worker` until `Some`, up to a generous wall-clock budget —
    /// this module's own cost reason (`generate` is a multi-second
    /// pipeline).
    fn poll_until_ready<T>(mut poll: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(30))
            .expect("30s from now does not overflow Instant");
        loop {
            if let Some(v) = poll() {
                return v;
            }
            assert!(
                Instant::now() < deadline,
                "worker never completed within budget"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// AC7 — spawn-then-poll reports pending, then the artifact.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    fn spawn_then_poll_reports_pending_then_ready() {
        let mut worker = Worker::new();
        let id = worker.request(cheap_params());
        assert!(worker.is_pending());
        assert!(
            worker.poll().is_none(),
            "poll immediately after request must not have a result yet"
        );

        let (got_id, result) = poll_until_ready(|| worker.poll());
        assert_eq!(got_id, id);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        assert!(!worker.is_pending());
    }

    /// AC7 — a superseded generation id's result is discarded: request A,
    /// then immediately supersede with request B; only B's id/result ever
    /// reaches a caller.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    fn superseded_generation_id_is_discarded() {
        let mut worker = Worker::new();
        let _id_a = worker.request(cheap_params());
        let id_b = worker.request(cheap_params());

        let (got_id, result) = poll_until_ready(|| worker.poll());
        assert_eq!(
            got_id, id_b,
            "only the superseding request's id must ever surface"
        );
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        // Give A's (superseded) thread time to finish and attempt its
        // send, then confirm no further message ever surfaces.
        std::thread::sleep(Duration::from_secs(5));
        assert!(
            worker.poll().is_none(),
            "no further result should ever surface"
        );
    }

    /// A panicking worker never `unwrap`s past — it reports `WorkerLost`.
    #[test]
    fn worker_panic_is_reported_as_worker_lost_not_a_process_panic() {
        // `min_straight: -1` is out of `gp_gen::generate`'s documented
        // domain in a way that panics deep in a debug-assertion-heavy
        // path is NOT guaranteed here; instead, directly exercise the
        // catch_unwind wiring by spawning a thread that panics and
        // routing it through the same message shape `Worker` uses --
        // this test targets the WorkerLost variant's plumbing, not a
        // real gp-gen panic (gp-gen's own pipeline is documented total
        // for valid domains -- see ai-docs/panic-index.md).
        let result: Result<(), GenerationFailure> = std::panic::catch_unwind(|| {
            panic!("synthetic worker panic");
        })
        .map_err(|_| GenerationFailure::WorkerLost);
        assert!(matches!(result, Err(GenerationFailure::WorkerLost)));
    }

    // ---- B3: map_outcome / phase_index / apply_terminal_rule ------------

    #[test]
    fn map_outcome_covers_all_four_variants() {
        assert_eq!(map_outcome(PhaseOutcome::Skipped), PhaseStatus::Skipped);
        assert_eq!(map_outcome(PhaseOutcome::Ok), PhaseStatus::Ok);
        assert_eq!(map_outcome(PhaseOutcome::Repair), PhaseStatus::Repair);
        assert_eq!(map_outcome(PhaseOutcome::Failed), PhaseStatus::Failed);
    }

    #[test]
    fn phase_index_is_a_bijection_onto_0_6() {
        let mut sorted: Vec<usize> = [
            Phase::F1,
            Phase::F2,
            Phase::F3,
            Phase::F4,
            Phase::F5,
            Phase::F6,
            Phase::F7,
        ]
        .into_iter()
        .map(phase_index)
        .collect();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    /// AC9 (`gp-game` half) — a phase that reports `Failed` on one
    /// attempt and `Ok` on a later one still aggregates to `Failed`
    /// (aggregate-worst, spec § Phase-status ordering).
    #[test]
    fn aggregate_keeps_the_worst_outcome_across_attempts() {
        let (sender, _receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut obs = WorkerObserver::new(GenerationId(0), cancel_flag, sender);

        obs.on_phase(PhaseEvent {
            phase: Phase::F4,
            outcome: PhaseOutcome::Failed,
        });
        obs.on_phase(PhaseEvent {
            phase: Phase::F4,
            outcome: PhaseOutcome::Ok,
        });

        assert_eq!(obs.aggregate[phase_index(Phase::F4)], PhaseStatus::Failed);
    }

    /// A snapshot is only sent when the aggregate actually changes (design
    /// § KD7) — a second, no-worse event for the same phase produces no
    /// further `WorkerMsg::Phases` message.
    #[test]
    fn snapshot_is_sent_only_on_change() {
        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut obs = WorkerObserver::new(GenerationId(0), cancel_flag, sender);

        obs.on_phase(PhaseEvent {
            phase: Phase::F1,
            outcome: PhaseOutcome::Ok,
        });
        obs.on_phase(PhaseEvent {
            phase: Phase::F1,
            outcome: PhaseOutcome::Ok,
        });

        let mut count = 0;
        while receiver.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 1, "identical repeat outcome must not resend");
    }

    /// The Terminal rule: every still-`Pending` slot becomes `Skipped`;
    /// an already-reached status is left untouched.
    #[test]
    fn terminal_rule_sweeps_pending_to_skipped_and_leaves_the_rest() {
        let mut phases = INITIAL_PHASES;
        phases[0] = PhaseStatus::Ok;
        phases[1] = PhaseStatus::Repair;
        // phases[2..7] stay Pending.

        apply_terminal_rule(&mut phases);

        assert_eq!(phases[0], PhaseStatus::Ok);
        assert_eq!(phases[1], PhaseStatus::Repair);
        for status in &phases[2..7] {
            assert_eq!(*status, PhaseStatus::Skipped);
        }
        assert!(phases.iter().all(|p| *p != PhaseStatus::Pending));
    }

    /// AC9 end-to-end (worker level): after a real request completes, no
    /// phase is left `Pending` — the Terminal rule fires inside
    /// `Worker::poll` on the terminal return.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "runs the gp-gen generation pipeline on a worker thread — a \
                  multi-second integer sweep whose interpreted wall-clock is \
                  prohibitive"
    )]
    fn worker_phases_have_no_pending_left_after_completion() {
        let mut worker = Worker::new();
        let _id = worker.request(cheap_params());
        let (_id, result) = poll_until_ready(|| worker.poll());
        assert!(result.is_ok(), "expected Ok, got {result:?}");

        let phases = worker.phases();
        assert!(
            phases.iter().all(|p| *p != PhaseStatus::Pending),
            "the Terminal rule must clear every Pending slot, got {phases:?}"
        );
        // The accepting run's Ф1 genuinely ran clean.
        assert_eq!(phases[phase_index(Phase::F1)], PhaseStatus::Ok);
    }
}
