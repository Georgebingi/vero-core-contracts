#![no_std]

mod guardian;
mod reputation;
mod task;
mod types;
pub mod events;

use soroban_sdk::{contract, contractimpl, Address, Env};
use types::{ContractError, DataKey};

pub use guardian::{add_guardian, is_guardian};
pub use task::{get_task, register_task};
pub use reputation::{calculate_voting_power, get_reputation, set_reputation};

/// Default weight threshold: a task requires at least 300 cumulative
/// reputation weight to be resolved. This can be overridden by the
/// admin via `set_weight_threshold`.
const DEFAULT_WEIGHT_THRESHOLD: u64 = 300;

#[contract]
pub struct VeroContract;

#[contractimpl]
impl VeroContract {
    // ─── Guardian management ───────────────────────────────────────

    pub fn add_guardian(env: Env, admin: Address, guardian: Address) {
        guardian::add_guardian(&env, admin, guardian);
    }

    // ─── Reputation management ─────────────────────────────────────

    /// Sets the reputation score for a guardian. Only callable by admin.
    pub fn set_reputation(env: Env, admin: Address, guardian: Address, score: u64) {
        reputation::set_reputation(&env, admin, guardian, score);
    }

    /// Returns the raw reputation score for a guardian.
    pub fn get_reputation(env: Env, guardian: Address) -> Option<u64> {
        reputation::get_reputation(&env, &guardian)
    }

    /// Calculates the voting power (weight) for a given guardian
    /// based on their reputation score.
    pub fn calculate_voting_power(env: Env, guardian: Address) -> Option<u64> {
        reputation::calculate_voting_power(&env, &guardian)
    }

    /// Sets the cumulative weight threshold required to resolve a task.
    /// Only callable by admin.
    pub fn set_weight_threshold(env: Env, admin: Address, threshold: u64) {
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::WeightThreshold, &threshold);
    }

    /// Returns the current weight threshold, falling back to the
    /// compiled default if none has been set.
    pub fn get_weight_threshold(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::WeightThreshold)
            .unwrap_or(DEFAULT_WEIGHT_THRESHOLD)
    }

    // ─── Task lifecycle ────────────────────────────────────────────

    pub fn register_task(
        env: Env,
        admin: Address,
        task_id: u64,
    ) -> Result<(), ContractError> {
        task::register_task(&env, admin, task_id)
    }

    /// Casts a weighted vote on a task. The guardian's reputation score
    /// determines their voting power. The vote weight is atomically
    /// added to the task's `total_weight_accrued`. Once the cumulative
    /// weight meets or exceeds the threshold, the task is resolved.
    ///
    /// # Errors
    /// * `NotAuthorized` — caller is not a registered guardian, or task not found.
    /// * `DuplicateVote` — guardian already voted on this task.
    /// * `NoReputationScore` — guardian has no reputation score assigned.
    /// * `ZeroWeightVote` — guardian's reputation score is zero.
    /// * `WeightOverflow` — adding the weight would overflow u64.
    pub fn vote(env: Env, guardian: Address, task_id: u64) -> Result<(), ContractError> {
        guardian.require_auth();

        // 1. Verify guardian status
        if !guardian::is_guardian(&env, &guardian) {
            return Err(ContractError::NotAuthorized);
        }

        // 2. Prevent duplicate votes
        let voted_key = DataKey::Voted(task_id, guardian.clone());
        if env.storage().instance().has(&voted_key) {
            return Err(ContractError::DuplicateVote);
        }

        // 3. Fetch voting power from reputation — single storage read
        let weight = reputation::calculate_voting_power(&env, &guardian)
            .ok_or(ContractError::NoReputationScore)?;

        if weight == 0 {
            return Err(ContractError::ZeroWeightVote);
        }

        // 4. Load the task — single storage read
        let task_key = DataKey::Task(task_id);
        let mut t: types::Task = env
            .storage()
            .instance()
            .get(&task_key)
            .ok_or(ContractError::NotAuthorized)?;

        // 5. Atomically increment weight with overflow protection
        t.total_weight_accrued = t
            .total_weight_accrued
            .checked_add(weight)
            .ok_or(ContractError::WeightOverflow)?;
        t.votes += 1;

        // 6. Check weight threshold for consensus
        let threshold: u64 = env
            .storage()
            .instance()
            .get(&DataKey::WeightThreshold)
            .unwrap_or(DEFAULT_WEIGHT_THRESHOLD);

        if t.total_weight_accrued >= threshold {
            t.is_done = true;
            events::emit_task_resolved(&env, task_id, t.total_weight_accrued);
        }

        // 7. Persist vote record and updated task — two storage writes
        env.storage().instance().set(&voted_key, &true);
        env.storage().instance().set(&task_key, &t);

        events::emit_weighted_vote(&env, task_id, &guardian, weight);

        Ok(())
    }

    pub fn get_task(env: Env, task_id: u64) -> Option<types::Task> {
        task::get_task(&env, task_id)
    }
}
