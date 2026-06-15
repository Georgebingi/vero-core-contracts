#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};
use vero_core_contracts::VeroContractClient;

fn setup() -> (Env, Address, VeroContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, vero_core_contracts::VeroContract);
    let client = VeroContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    (env, admin, client)
}

#[test]
fn test_add_guardian_and_register_task() {
    let (_env, admin, client) = setup();
    let guardian = Address::generate(&_env);

    client.add_guardian(&admin, &guardian);
    client.register_task(&admin, &1u64);

    let task = client.get_task(&1u64).unwrap();
    assert_eq!(task.id, 1);
    assert_eq!(task.votes, 0);
    assert!(!task.is_done);
}

#[test]
fn test_three_votes_flips_is_done() {
    let (env, admin, client) = setup();

    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);

    client.add_guardian(&admin, &g1);
    client.add_guardian(&admin, &g2);
    client.add_guardian(&admin, &g3);
    client.register_task(&admin, &42u64);

    client.vote(&g1, &42u64);
    client.vote(&g2, &42u64);
    client.vote(&g3, &42u64);

    let task = client.get_task(&42u64).unwrap();
    assert_eq!(task.votes, 3);
    assert!(task.is_done);
}

#[test]
fn test_duplicate_vote_rejected() {
    let (env, admin, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    client.register_task(&admin, &7u64);
    client.vote(&g, &7u64);

    let result = client.try_vote(&g, &7u64);
    assert!(result.is_err());
}

#[test]
fn test_non_guardian_vote_rejected() {
    let (env, admin, client) = setup();
    let stranger = Address::generate(&env);

    client.register_task(&admin, &99u64);

    let result = client.try_vote(&stranger, &99u64);
    assert!(result.is_err());
}

// ─── Drips cross-contract integration tests ────────────────────────────

#[test]
fn test_reward_stream_rejected_for_unverified_task() {
    let (env, admin, client) = setup();
    let contributor = Address::generate(&env);
    let drips_addr = Address::generate(&env);

    // Register but do NOT verify the task (no votes)
    client.register_task(&admin, &10u64);

    let result = client.try_start_reward_stream(&admin, &drips_addr, &contributor, &10u64);
    assert!(result.is_err(), "should reject stream for unverified task");
}

#[test]
fn test_reward_stream_rejected_for_nonexistent_task() {
    let (env, admin, client) = setup();
    let contributor = Address::generate(&env);
    let drips_addr = Address::generate(&env);

    // Task 999 was never registered
    let result = client.try_start_reward_stream(&admin, &drips_addr, &contributor, &999u64);
    assert!(result.is_err(), "should reject stream for nonexistent task");
}

#[test]
fn test_reward_stream_duplicate_rejected() {
    let (env, admin, client) = setup();
    let contributor = Address::generate(&env);

    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);

    client.add_guardian(&admin, &g1);
    client.add_guardian(&admin, &g2);
    client.add_guardian(&admin, &g3);
    client.register_task(&admin, &50u64);

    client.vote(&g1, &50u64);
    client.vote(&g2, &50u64);
    client.vote(&g3, &50u64);

    // Deploy a mock Drips contract to receive the cross-contract call
    let drips_contract_id = env.register_contract(None, MockDripsContract);

    // First stream should succeed
    client.start_reward_stream(&admin, &drips_contract_id, &contributor, &50u64);

    // Second attempt for same task should fail
    let result =
        client.try_start_reward_stream(&admin, &drips_contract_id, &contributor, &50u64);
    assert!(result.is_err(), "should reject duplicate stream");
}

#[test]
fn test_reward_stream_stored_after_success() {
    let (env, admin, client) = setup();
    let contributor = Address::generate(&env);

    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);

    client.add_guardian(&admin, &g1);
    client.add_guardian(&admin, &g2);
    client.add_guardian(&admin, &g3);
    client.register_task(&admin, &77u64);

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

// ─── Re-entrancy protection tests ─────────────────────────────────────

#[test]
fn test_lock_released_after_successful_vote() {
    // After a normal vote the lock must be cleared so subsequent votes work.
    // If the lock leaked, the second vote would fail with Locked.
    let (env, admin, client) = setup();
    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);

    client.add_guardian(&admin, &g1);
    client.add_guardian(&admin, &g2);
    client.register_task(&admin, &202u64);

    client.vote(&g1, &202u64);
    client.vote(&g2, &202u64); // would fail if lock was not released

    let task = client.get_task(&202u64).unwrap();
    assert_eq!(task.votes, 2);
}

#[test]
fn test_lock_released_after_successful_register_task() {
    // Registering two tasks sequentially verifies the lock is released each time.
    let (_env, admin, client) = setup();

    client.register_task(&admin, &300u64);
    client.register_task(&admin, &301u64); // would fail if lock leaked

    assert!(client.get_task(&300u64).is_some());
    assert!(client.get_task(&301u64).is_some());
}

#[test]
fn test_lock_released_after_failed_vote() {
    // When vote() fails early (non-guardian), the lock must still be released
    // so a subsequent legitimate call can succeed.
    let (env, admin, client) = setup();
    let g = Address::generate(&env);
    let stranger = Address::generate(&env);

    client.add_guardian(&admin, &g);
    client.register_task(&admin, &303u64);

    // Non-guardian vote is rejected (lock must be released inside)
    let _ = client.try_vote(&stranger, &303u64);

    // Legitimate vote must still succeed
    client.vote(&g, &303u64);
    assert_eq!(client.get_task(&303u64).unwrap().votes, 1);
}

// ─── Mock Drips contract for test isolation ────────────────────────────

use soroban_sdk::{contract, contractimpl};

/// A minimal mock of the Drips protocol contract used in tests.
/// It accepts `start_stream` calls without side-effects so we can
/// validate the Vero contract's cross-contract call logic in isolation.
#[contract]
pub struct MockDripsContract;

#[contractimpl]
impl MockDripsContract {
    pub fn start_stream(
        _env: Env,
        _contributor: Address,
        _task_id: u64,
        _resolution_status: u32,
    ) {
        // Mock: accept the call silently
    }
}
