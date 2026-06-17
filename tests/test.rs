#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env,
};
use vero_core_contracts::VeroContractClient;

// Matches the lock threshold set in setup().
const LOCK_THRESHOLD: i128 = 100;

fn setup() -> (Env, Address, Address, VeroContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, vero_core_contracts::VeroContract);
    let client = VeroContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin.clone());
    let token_addr = token.address();

    client.initialize(&token_addr, &LOCK_THRESHOLD);

    (env, admin, token_addr, client)
}

fn add_guardian_with_rep(
    env: &Env,
    client: &VeroContractClient,
    admin: &Address,
    score: u64,
) -> Address {
    let g = Address::generate(env);
    client.add_guardian(admin, &g);
    client.set_reputation(admin, &g, &score);
    g
}

fn lock_for_guardian(
    env: &Env,
    token: &Address,
    client: &VeroContractClient,
    guardian: &Address,
    amount: i128,
) {
    let asset_client = soroban_sdk::token::StellarAssetClient::new(env, token);
    asset_client.mint(guardian, &amount);
    client.lock_tokens(guardian, &amount);
}

// ─── Admin initialisation ───────────────────────────────────────────

#[test]
fn test_admin_is_persisted_on_initialize() {
    let (_, admin, _, client) = setup();
    assert_eq!(client.get_admin(), Some(admin));
}

#[test]
fn test_non_admin_cannot_register_task() {
    let (env, _admin, _token, client) = setup();
    let stranger = Address::generate(&env);

    // stranger is not the stored admin — must be rejected
    let result = client.try_register_task(&stranger, &1u64);
    assert!(result.is_err());
}

#[test]
fn test_admin_can_register_task() {
    let (_env, admin, _token, client) = setup();
    client.register_task(&admin, &1u64);
    assert!(client.get_task(&1u64).is_some());
}

#[test]
fn test_register_task_rejected_before_initialize() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, vero_core_contracts::VeroContract);
    let client = VeroContractClient::new(&env, &contract_id);
    let stranger = Address::generate(&env);

    // No initialize called → no stored admin → NotInitialized
    let result = client.try_register_task(&stranger, &1u64);
    assert!(result.is_err());
}

// ─── Basic contract state ───────────────────────────────────────────

#[test]
fn test_add_guardian_and_register_task() {
    let (env, admin, _token, client) = setup();
    let guardian = Address::generate(&env);

    client.add_guardian(&admin, &guardian);
    client.register_task(&admin, &1u64);

    let task = client.get_task(&1u64).unwrap();
    assert_eq!(task.id, 1);
    assert_eq!(task.votes, 0);
    assert_eq!(task.total_weight_accrued, 0);
    assert_eq!(task.resolved_at, 0);
    assert!(!task.is_done);
}

#[test]
fn test_set_and_get_reputation() {
    let (env, admin, _token, client) = setup();
    let guardian = Address::generate(&env);

    client.add_guardian(&admin, &guardian);
    client.set_reputation(&admin, &guardian, &500u64);

    assert_eq!(client.get_reputation(&guardian), Some(500));
    assert_eq!(client.calculate_voting_power(&guardian), Some(500));
}

#[test]
fn test_calculate_voting_power_returns_score() {
    let (env, admin, _token, client) = setup();
    let guardian = Address::generate(&env);

    client.add_guardian(&admin, &guardian);
    client.set_reputation(&admin, &guardian, &150u64);

    assert_eq!(client.calculate_voting_power(&guardian), Some(150));
}

#[test]
fn test_calculate_voting_power_none_for_unset() {
    let (env, _admin, _token, client) = setup();
    let stranger = Address::generate(&env);

    assert_eq!(client.calculate_voting_power(&stranger), None);
}

// ─── Weighted consensus ─────────────────────────────────────────────

#[test]
fn test_single_high_rep_guardian_resolves_task() {
    let (env, admin, token, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let g = add_guardian_with_rep(&env, &client, &admin, 300);
    client.register_task(&admin, &1u64);
    lock_for_guardian(&env, &token, &client, &g, 101);
    client.vote(&g, &1u64);

    let task = client.get_task(&1u64).unwrap();
    assert_eq!(task.votes, 1);
    assert_eq!(task.total_weight_accrued, 300);
    assert!(task.is_done);
}

#[test]
fn test_multiple_guardians_accumulate_weight() {
    let (env, admin, token, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g3 = add_guardian_with_rep(&env, &client, &admin, 100);

    client.register_task(&admin, &42u64);

    lock_for_guardian(&env, &token, &client, &g1, 101);
    lock_for_guardian(&env, &token, &client, &g2, 101);
    lock_for_guardian(&env, &token, &client, &g3, 101);

    client.vote(&g1, &42u64);
    client.vote(&g2, &42u64);
    client.vote(&g3, &42u64);

    let task = client.get_task(&42u64).unwrap();
    assert_eq!(task.votes, 3);
    assert!(task.is_done);
}

#[test]
fn test_weight_vs_count_logic() {
    let (env, admin, token, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 200);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 150);

    client.register_task(&admin, &20u64);

    lock_for_guardian(&env, &token, &client, &g1, 101);
    lock_for_guardian(&env, &token, &client, &g2, 101);

    client.vote(&g1, &20u64);
    client.vote(&g2, &20u64);

    let task = client.get_task(&20u64).unwrap();
    assert_eq!(task.votes, 2);
    assert_eq!(task.total_weight_accrued, 350);
    assert!(task.is_done);
}

#[test]
fn test_insufficient_weight_does_not_resolve_task() {
    // Five guardians each with rep=100, threshold=600 → total 500 < 600, not done.
    let (env, admin, token, client) = setup();
    client.set_weight_threshold(&admin, &600u64);

    let guardians: [Address; 5] = core::array::from_fn(|_| {
        add_guardian_with_rep(&env, &client, &admin, 100)
    });

    client.register_task(&admin, &10u64);

    for g in &guardians {
        lock_for_guardian(&env, &token, &client, g, 101);
        client.vote(g, &10u64);
    }

    let task = client.get_task(&10u64).unwrap();
    assert_eq!(task.votes, 5);
    assert_eq!(task.total_weight_accrued, 500);
    assert!(!task.is_done);
}

#[test]
fn test_task_resolved_includes_final_weight() {
    let (env, admin, token, client) = setup();
    client.set_weight_threshold(&admin, &200u64);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 115);

    client.register_task(&admin, &40u64);

    lock_for_guardian(&env, &token, &client, &g1, 101);
    lock_for_guardian(&env, &token, &client, &g2, 101);

    client.vote(&g1, &40u64);
    client.vote(&g2, &40u64);

    let task = client.get_task(&40u64).unwrap();
    assert_eq!(task.total_weight_accrued, 215);
    assert!(task.is_done);
}

#[test]
fn test_custom_weight_threshold() {
    let (_env, admin, _token, client) = setup();

    assert_eq!(client.get_weight_threshold(), 300);

    client.set_weight_threshold(&admin, &1000u64);
    assert_eq!(client.get_weight_threshold(), 1000);
}

// ─── Reputation gate ────────────────────────────────────────────────

#[test]
fn test_vote_rejected_without_reputation() {
    let (env, admin, token, client) = setup();
    let g = Address::generate(&env);
    client.add_guardian(&admin, &g);
    client.register_task(&admin, &7u64);
    lock_for_guardian(&env, &token, &client, &g, LOCK_THRESHOLD + 1);

    // No reputation set → NoReputationScore
    let result = client.try_vote(&g, &7u64);
    assert!(result.is_err());
}

#[test]
fn test_vote_rejected_with_insufficient_reputation() {
    let (env, admin, token, client) = setup();
    // Score 99 is below MIN_REPUTATION_THRESHOLD (100)
    let g = add_guardian_with_rep(&env, &client, &admin, 99);
    client.register_task(&admin, &8u64);
    lock_for_guardian(&env, &token, &client, &g, 101);

    let result = client.try_vote(&g, &8u64);
    assert!(result.is_err());
}

#[test]
fn test_vote_allowed_at_minimum_reputation_threshold() {
    let (env, admin, token, client) = setup();
    // Score 100 is exactly at MIN_REPUTATION_THRESHOLD — should be allowed
    let g = add_guardian_with_rep(&env, &client, &admin, 100);
    client.register_task(&admin, &9u64);
    lock_for_guardian(&env, &token, &client, &g, 101);

    let result = client.try_vote(&g, &9u64);
    assert!(result.is_ok());
}

#[test]
fn test_vote_on_nonexistent_task_rejected() {
    let (env, admin, _token, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 100);

    let result = client.try_vote(&g, &999u64);
    assert!(result.is_err());
}

// ─── Reputation update ──────────────────────────────────────────────

#[test]
fn test_reputation_can_be_updated() {
    let (env, admin, _token, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    client.set_reputation(&admin, &g, &100u64);
    assert_eq!(client.get_reputation(&g), Some(100));

    client.set_reputation(&admin, &g, &500u64);
    assert_eq!(client.get_reputation(&g), Some(500));
    assert_eq!(client.calculate_voting_power(&g), Some(500));
}

// ─── Drips integration ─────────────────────────────────────────────

#[test]
fn test_reward_stream_rejected_until_task_verified() {
    let (env, admin, _token, client) = setup();
    let contributor = Address::generate(&env);
    let drips_addr = Address::generate(&env);

    let result = client.try_start_reward_stream(&admin, &drips_addr, &contributor, &999u64);
    assert!(result.is_err());
}

#[test]
fn test_reward_stream_duplicate_rejected() {
    let (env, admin, token, client) = setup();
    let contributor = Address::generate(&env);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g3 = add_guardian_with_rep(&env, &client, &admin, 100);
    client.register_task(&admin, &50u64);

    lock_for_guardian(&env, &token, &client, &g1, 101);
    lock_for_guardian(&env, &token, &client, &g2, 101);
    lock_for_guardian(&env, &token, &client, &g3, 101);

    client.vote(&g1, &50u64);
    client.vote(&g2, &50u64);
    client.vote(&g3, &50u64);

    let drips_contract_id = env.register_contract(None, MockDripsContract);

    client.start_reward_stream(&admin, &drips_contract_id, &contributor, &50u64);

    let result = client.try_start_reward_stream(&admin, &drips_contract_id, &contributor, &50u64);
    assert!(result.is_err());
}

#[test]
fn test_reward_stream_stored_after_success() {
    let (env, admin, token, client) = setup();
    let contributor = Address::generate(&env);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g3 = add_guardian_with_rep(&env, &client, &admin, 100);
    client.register_task(&admin, &77u64);

    lock_for_guardian(&env, &token, &client, &g1, 101);
    lock_for_guardian(&env, &token, &client, &g2, 101);
    lock_for_guardian(&env, &token, &client, &g3, 101);

    client.vote(&g1, &77u64);
    client.vote(&g2, &77u64);
    client.vote(&g3, &77u64);

    let drips_contract_id = env.register_contract(None, MockDripsContract);
    client.start_reward_stream(&admin, &drips_contract_id, &contributor, &77u64);

    let stream = client.get_reward_stream(&77u64).unwrap();
    assert_eq!(stream.task_id, 77);
    assert_eq!(stream.contributor, contributor);
    assert!(stream.active);
}

// ─── Token locking ─────────────────────────────────────────────────

#[test]
fn test_voting_fails_if_tokens_not_locked() {
    let (env, admin, _token, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    client.set_reputation(&admin, &g, &100u64);
    client.register_task(&admin, &100u64);

    let result = client.try_vote(&g, &100u64);
    assert!(result.is_err());
}

#[test]
fn test_locked_balance_must_exceed_threshold() {
    let (env, admin, token, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    client.set_reputation(&admin, &g, &100u64);
    client.register_task(&admin, &100u64);

    lock_for_guardian(&env, &token, &client, &g, 100);
    assert!(client.try_vote(&g, &100u64).is_err());

    lock_for_guardian(&env, &token, &client, &g, 1);
    client.vote(&g, &100u64);
    assert_eq!(client.get_task(&100u64).unwrap().votes, 1);
}

#[test]
fn test_resign_guardian_refunds_tokens() {
    let (env, admin, token, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    lock_for_guardian(&env, &token, &client, &g, 200);

    client.resign_guardian(&g);

    assert!(!client.is_guardian(&g));
    let token_client = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&g), 200);
}

#[test]
fn test_unlock_fails_while_guardian() {
    let (env, admin, token, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    lock_for_guardian(&env, &token, &client, &g, 200);

    assert!(client.try_unlock_tokens(&g).is_err());
}

#[test]
fn test_unlock_succeeds_for_non_guardian() {
    let (env, _admin, token, client) = setup();
    let non_guardian = Address::generate(&env);

    lock_for_guardian(&env, &token, &client, &non_guardian, 150);
    client.unlock_tokens(&non_guardian);

    let token_client = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&non_guardian), 150);
}

// ─── Re-entrancy protection ─────────────────────────────────────────

#[test]
fn test_lock_released_after_successful_vote() {
    let (env, admin, token, client) = setup();
    let g1 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 100);

    client.register_task(&admin, &202u64);

    lock_for_guardian(&env, &token, &client, &g1, 101);
    lock_for_guardian(&env, &token, &client, &g2, 101);

    client.vote(&g1, &202u64);
    client.vote(&g2, &202u64);

    assert_eq!(client.get_task(&202u64).unwrap().votes, 2);
}

#[test]
fn test_lock_released_after_successful_register_task() {
    let (_env, admin, _token, client) = setup();

    client.register_task(&admin, &300u64);
    client.register_task(&admin, &301u64);

    assert!(client.get_task(&300u64).is_some());
    assert!(client.get_task(&301u64).is_some());
}

#[test]
fn test_lock_released_after_failed_vote() {
    let (env, admin, token, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 100);
    let stranger = Address::generate(&env);

    client.register_task(&admin, &303u64);

    let _ = client.try_vote(&stranger, &303u64);

    lock_for_guardian(&env, &token, &client, &g, 101);
    client.vote(&g, &303u64);
    assert_eq!(client.get_task(&303u64).unwrap().votes, 1);
}

// ─── Emergency stop (pause/unpause) ────────────────────────────────

#[test]
fn test_admin_can_toggle_pause() {
    let (_env, admin, _token, client) = setup();

    assert!(!client.is_paused());
    client.toggle_pause(&admin);
    assert!(client.is_paused());
    client.toggle_pause(&admin);
    assert!(!client.is_paused());
}

#[test]
fn test_admin_can_pause_and_unpause() {
    let (_env, admin, _token, client) = setup();

    client.pause(&admin);
    assert!(client.is_paused());
    client.unpause(&admin);
    assert!(!client.is_paused());
}

#[test]
fn test_contract_paused_error_on_register_task() {
    let (_env, admin, _token, client) = setup();

    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());

    assert!(client.try_register_task(&admin, &1u64).is_err());
}

#[test]
fn test_paused_contract_rejects_register_task() {
    let (_env, admin, _token, client) = setup();
    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());

    assert!(client.try_register_task(&admin, &2u64).is_err());
}

#[test]
fn test_contract_paused_error_on_vote() {
    let (env, admin, token, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 300);
    client.register_task(&admin, &1u64);
    lock_for_guardian(&env, &token, &client, &g, 101);

    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());

    assert!(client.try_vote(&g, &1u64).is_err());
}

#[test]
fn test_contract_paused_error_on_add_guardian() {
    let (env, admin, _token, client) = setup();
    let guardian = Address::generate(&env);

    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());

    assert!(client.try_add_guardian(&admin, &guardian).is_err());
}

#[test]
fn test_contract_paused_error_on_set_reputation() {
    let (env, admin, _token, client) = setup();
    let guardian = Address::generate(&env);
    client.add_guardian(&admin, &guardian);

    client.toggle_pause(&admin);

    assert!(client.try_set_reputation(&admin, &guardian, &100u64).is_err());
}

#[test]
fn test_operations_resume_after_unpause() {
    let (env, admin, token, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 300);

    client.toggle_pause(&admin);
    assert!(client.is_paused());

    // Verify operations are rejected while paused
    assert!(client.try_register_task(&admin, &1u64).is_err());

    client.toggle_pause(&admin);
    assert!(!client.is_paused());

    // Operations succeed after unpause
    client.register_task(&admin, &1u64);
    lock_for_guardian(&env, &token, &client, &g, 101);
    client.vote(&g, &1u64);

    let task = client.get_task(&1u64).unwrap();
    assert!(task.is_done);
}

#[test]
fn test_explicit_pause_and_unpause_rejects_vote() {
    let (env, admin, token, client) = setup();
    client.register_task(&admin, &1u64);
    let g = add_guardian_with_rep(&env, &client, &admin, 100);
    lock_for_guardian(&env, &token, &client, &g, 101);

    client.pause(&admin);
    assert!(client.is_paused());

    let result = client.try_vote(&g, &1u64);
    assert!(result.is_err());

    client.unpause(&admin);
    assert!(!client.is_paused());
    client.register_task(&admin, &2u64);
    assert!(client.get_task(&2u64).is_some());
}

#[test]
fn test_paused_contract_rejects_vote() {
    let (env, admin, token, client) = setup();
    client.register_task(&admin, &1u64);
    let g = add_guardian_with_rep(&env, &client, &admin, 100);
    lock_for_guardian(&env, &token, &client, &g, 101);

    client.pause(&admin);
    assert!(client.is_paused());

    assert!(client.try_vote(&g, &1u64).is_err());
}

// ─── Mock Drips contract ───────────────────────────────────────────

use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct MockDripsContract;

#[contractimpl]
impl MockDripsContract {
    pub fn start_stream(
        _env: Env,
        _contributor: Address,
        _task_id: u64,
        _resolution_status: u32,
    ) {}
}

// ─── Circuit breaker ───────────────────────────────────────────────

#[test]
fn test_circuit_breaker_trips_after_threshold() {
    let (_env, _admin, _token, client) = setup();
    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());
}

#[test]
fn test_paused_contract_rejects_vote_via_circuit_breaker() {
    let (env, admin, token, client) = setup();
    client.register_task(&admin, &1u64);
    let g = add_guardian_with_rep(&env, &client, &admin, 100);
    lock_for_guardian(&env, &token, &client, &g, 101);

    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());

    assert!(client.try_vote(&g, &1u64).is_err());
}

#[test]
fn test_admin_can_reset_circuit_breaker() {
    let (env, admin, token, client) = setup();
    client.register_task(&admin, &1u64);
    let g = add_guardian_with_rep(&env, &client, &admin, 100);
    lock_for_guardian(&env, &token, &client, &g, 101);

    for _ in 0..51 {
        client.record_failure();
    }
    assert!(client.is_paused());

    client.reset_circuit_breaker(&admin);
    assert!(!client.is_paused());

    assert!(client.try_vote(&g, &1u64).is_ok());
}

#[test]
fn debug_circuit_breaker_count() {
    let (_env, _admin, _token, client) = setup();
    for _ in 0..50 {
        client.record_failure();
    }
    assert!(!client.is_paused());

    client.record_failure();
    assert!(client.is_paused());
}

// ─── Gas cost estimation ───────────────────────────────────────────

use vero_core_contracts::Operation;

#[test]
fn test_all_operations_return_nonzero_cost() {
    let (_env, _admin, _token, client) = setup();

    let ops = [
        Operation::RegisterTask, Operation::Vote, Operation::AddGuardian,
        Operation::SetReputation, Operation::LockTokens, Operation::UnlockTokens,
        Operation::ResignGuardian, Operation::SetWeightThreshold, Operation::StartRewardStream,
        Operation::TogglePause, Operation::RecordFailure, Operation::ResetCircuitBreaker,
        Operation::UpgradeContract,
    ];

    for op in ops {
        assert!(client.get_estimated_cost(&op) > 0, "{:?} returned zero cost", op);
    }
}

#[test]
fn test_vote_is_most_expensive_write_operation() {
    let (_env, _admin, _token, client) = setup();
    let vote_cost = client.get_estimated_cost(&Operation::Vote);

    let ops = [
        Operation::RegisterTask, Operation::AddGuardian, Operation::SetReputation,
        Operation::LockTokens, Operation::UnlockTokens, Operation::ResignGuardian,
        Operation::SetWeightThreshold, Operation::StartRewardStream, Operation::TogglePause,
        Operation::RecordFailure, Operation::ResetCircuitBreaker,
    ];

    for op in ops {
        assert!(vote_cost >= client.get_estimated_cost(&op), "Vote should be >= {:?}", op);
    }
}

#[test]
fn test_upgrade_contract_is_overall_maximum() {
    let (_env, _admin, _token, client) = setup();
    let upgrade_cost = client.get_estimated_cost(&Operation::UpgradeContract);

    let ops = [
        Operation::RegisterTask, Operation::Vote, Operation::AddGuardian,
        Operation::SetReputation, Operation::LockTokens, Operation::UnlockTokens,
        Operation::ResignGuardian, Operation::SetWeightThreshold, Operation::StartRewardStream,
        Operation::TogglePause, Operation::RecordFailure, Operation::ResetCircuitBreaker,
    ];

    for op in ops {
        assert!(upgrade_cost >= client.get_estimated_cost(&op), "UpgradeContract should be >= {:?}", op);
    }
}

#[test]
fn test_cost_spot_checks() {
    let (_env, _admin, _token, client) = setup();

    assert_eq!(client.get_estimated_cost(&Operation::SetWeightThreshold), 650_000);
    assert_eq!(client.get_estimated_cost(&Operation::SetReputation),       700_000);
    assert_eq!(client.get_estimated_cost(&Operation::AddGuardian),         700_000);
    assert_eq!(client.get_estimated_cost(&Operation::TogglePause),         730_000);
    assert_eq!(client.get_estimated_cost(&Operation::ResetCircuitBreaker), 800_000);
    assert_eq!(client.get_estimated_cost(&Operation::RecordFailure),       880_000);
    assert_eq!(client.get_estimated_cost(&Operation::RegisterTask),      1_000_000);

    assert_eq!(client.get_estimated_cost(&Operation::LockTokens),        1_250_000);
    assert_eq!(client.get_estimated_cost(&Operation::StartRewardStream), 1_330_000);
    assert_eq!(client.get_estimated_cost(&Operation::UnlockTokens),      1_300_000);
    assert_eq!(client.get_estimated_cost(&Operation::ResignGuardian),    1_400_000);

    assert_eq!(client.get_estimated_cost(&Operation::Vote),              1_960_000);
    assert_eq!(client.get_estimated_cost(&Operation::UpgradeContract),   2_500_000);
}

#[test]
fn test_estimated_cost_requires_no_auth() {
    let env = Env::default();
    let contract_id = env.register_contract(None, vero_core_contracts::VeroContract);
    let client = VeroContractClient::new(&env, &contract_id);

    let cost = client.get_estimated_cost(&Operation::Vote);
    assert!(cost > 0);
}

#[test]
fn test_all_costs_above_base_invocation_overhead() {
    let (_env, _admin, _token, client) = setup();
    const BASE: u64 = 500_000;

    let ops = [
        Operation::RegisterTask, Operation::Vote, Operation::AddGuardian,
        Operation::SetReputation, Operation::LockTokens, Operation::UnlockTokens,
        Operation::ResignGuardian, Operation::SetWeightThreshold, Operation::StartRewardStream,
        Operation::TogglePause, Operation::RecordFailure, Operation::ResetCircuitBreaker,
        Operation::UpgradeContract,
    ];

    for op in ops {
        assert!(client.get_estimated_cost(&op) > BASE, "{:?} is below base overhead", op);
    }
}
