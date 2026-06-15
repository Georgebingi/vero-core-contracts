// Event emission helpers for contract state transitions.

use soroban_sdk::{symbol_short, Address, Env};

/// Emits an event when a weighted vote is cast by a guardian.
///
/// Event topic: `"wt_vote"` (weighted_vote)
/// Event data: `(task_id, guardian_address, weight)`
pub fn emit_weighted_vote(env: &Env, task_id: u64, guardian: &Address, weight: u64) {
    env.events().publish(
        (symbol_short!("wt_vote"),),
        (task_id, guardian.clone(), weight),
    );
}

/// Emits an event when a task reaches the weight threshold and is resolved.
///
/// Event topic: `"resolved"` (task_resolved)
/// Event data: `(task_id, total_weight_accrued)`
pub fn emit_task_resolved(env: &Env, task_id: u64, total_weight_accrued: u64) {
    env.events()
        .publish((symbol_short!("resolved"),), (task_id, total_weight_accrued));
}
