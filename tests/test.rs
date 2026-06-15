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

/// Helper: creates a guardian with a given reputation score.
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

// ─── Basic guardian & task registration (unchanged behaviour) ──────

#[test]
fn test_add_guardian_and_register_task() {
    let (env, admin, client) = setup();
    let guardian = Address::generate(&env);

    client.add_guardian(&admin, &guardian);
    client.register_task(&admin, &1u64);

    let task = client.get_task(&1u64).unwrap();
    assert_eq!(task.id, 1);
    assert_eq!(task.votes, 0);
    assert_eq!(task.total_weight_accrued, 0);
    assert!(!task.is_done);
}

// ─── Reputation management ─────────────────────────────────────────

#[test]
fn test_set_and_get_reputation() {
    let (env, admin, client) = setup();
    let guardian = Address::generate(&env);

    client.add_guardian(&admin, &guardian);
    client.set_reputation(&admin, &guardian, &500u64);

    let score = client.get_reputation(&guardian);
    assert_eq!(score, Some(500));
}

#[test]
fn test_calculate_voting_power_returns_score() {
    let (env, admin, client) = setup();
    let guardian = Address::generate(&env);

    client.add_guardian(&admin, &guardian);
    client.set_reputation(&admin, &guardian, &150u64);

    let power = client.calculate_voting_power(&guardian);
    assert_eq!(power, Some(150));
}

#[test]
fn test_calculate_voting_power_none_for_unset() {
    let (env, _admin, client) = setup();
    let stranger = Address::generate(&env);

    let power = client.calculate_voting_power(&stranger);
    assert_eq!(power, None);
}

// ─── Weighted consensus: weight-based resolution ───────────────────

#[test]
fn test_single_high_rep_guardian_resolves_task() {
    // A single guardian with reputation >= threshold can resolve a task alone
    let (env, admin, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let g = add_guardian_with_rep(&env, &client, &admin, 300);
    client.register_task(&admin, &1u64);
    client.vote(&g, &1u64);

    let task = client.get_task(&1u64).unwrap();
    assert_eq!(task.votes, 1);
    assert_eq!(task.total_weight_accrued, 300);
    assert!(task.is_done, "single high-rep guardian should resolve task");
}

#[test]
fn test_multiple_low_rep_guardians_accumulate_weight() {
    // Three guardians with rep=100 each → total_weight = 300 → resolved
    let (env, admin, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 100);
    let g3 = add_guardian_with_rep(&env, &client, &admin, 100);

    client.register_task(&admin, &10u64);

    client.vote(&g1, &10u64);
    let task = client.get_task(&10u64).unwrap();
    assert_eq!(task.total_weight_accrued, 100);
    assert!(!task.is_done);

    client.vote(&g2, &10u64);
    let task = client.get_task(&10u64).unwrap();
    assert_eq!(task.total_weight_accrued, 200);
    assert!(!task.is_done);

    client.vote(&g3, &10u64);
    let task = client.get_task(&10u64).unwrap();
    assert_eq!(task.total_weight_accrued, 300);
    assert_eq!(task.votes, 3);
    assert!(task.is_done, "three low-rep guardians should resolve task");
}

#[test]
fn test_weight_vs_count_logic() {
    // Two guardians with high rep should resolve even though count < 3.
    // This demonstrates weight-based consensus vs the old count-based system.
    let (env, admin, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 200);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 150);

    client.register_task(&admin, &20u64);

    client.vote(&g1, &20u64);
    client.vote(&g2, &20u64);

    let task = client.get_task(&20u64).unwrap();
    assert_eq!(task.votes, 2, "only 2 votes cast");
    assert_eq!(task.total_weight_accrued, 350);
    assert!(
        task.is_done,
        "2 high-rep votes should resolve task despite count < 3"
    );
}

#[test]
fn test_many_low_rep_guardians_cannot_resolve_without_enough_weight() {
    // Five guardians with rep=50 each → total_weight = 250 < 300 → NOT resolved
    let (env, admin, client) = setup();
    client.set_weight_threshold(&admin, &300u64);

    let guardians: [Address; 5] = core::array::from_fn(|_| {
        add_guardian_with_rep(&env, &client, &admin, 50)
    });

    client.register_task(&admin, &30u64);

    for g in &guardians {
        client.vote(g, &30u64);
    }

    let task = client.get_task(&30u64).unwrap();
    assert_eq!(task.votes, 5);
    assert_eq!(task.total_weight_accrued, 250);
    assert!(
        !task.is_done,
        "5 guardians with rep=50 should NOT reach threshold of 300"
    );
}

#[test]
fn test_task_resolved_includes_final_weight() {
    // Verify the resolved task's total_weight_accrued reflects the exact sum
    let (env, admin, client) = setup();
    client.set_weight_threshold(&admin, &100u64);

    let g1 = add_guardian_with_rep(&env, &client, &admin, 42);
    let g2 = add_guardian_with_rep(&env, &client, &admin, 73);

    client.register_task(&admin, &40u64);
    client.vote(&g1, &40u64);
    client.vote(&g2, &40u64);

    let task = client.get_task(&40u64).unwrap();
    assert_eq!(task.total_weight_accrued, 115, "42 + 73 = 115");
    assert!(task.is_done);
}

// ─── Configurable weight threshold ─────────────────────────────────

#[test]
fn test_custom_weight_threshold() {
    let (_env, admin, client) = setup();

    // Default threshold
    let default = client.get_weight_threshold();
    assert_eq!(default, 300);

    // Set custom threshold
    client.set_weight_threshold(&admin, &1000u64);
    assert_eq!(client.get_weight_threshold(), 1000);
}

// ─── Error handling ─────────────────────────────────────────────────

#[test]
fn test_vote_rejected_without_reputation() {
    let (env, admin, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    // No reputation set
    client.register_task(&admin, &50u64);

    let result = client.try_vote(&g, &50u64);
    assert!(result.is_err(), "vote without reputation should be rejected");
}

#[test]
fn test_vote_rejected_with_zero_reputation() {
    let (env, admin, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 0);

    client.register_task(&admin, &51u64);

    let result = client.try_vote(&g, &51u64);
    assert!(
        result.is_err(),
        "vote with zero reputation should be rejected"
    );
}

#[test]
fn test_duplicate_vote_rejected() {
    let (env, admin, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 100);

    client.register_task(&admin, &7u64);
    client.vote(&g, &7u64);

    let result = client.try_vote(&g, &7u64);
    assert!(result.is_err(), "duplicate vote should be rejected");
}

#[test]
fn test_non_guardian_vote_rejected() {
    let (env, admin, client) = setup();
    let stranger = Address::generate(&env);

    client.register_task(&admin, &99u64);

    let result = client.try_vote(&stranger, &99u64);
    assert!(result.is_err(), "non-guardian vote should be rejected");
}

#[test]
fn test_vote_on_nonexistent_task_rejected() {
    let (env, admin, client) = setup();
    let g = add_guardian_with_rep(&env, &client, &admin, 100);

    let result = client.try_vote(&g, &999u64);
    assert!(
        result.is_err(),
        "vote on nonexistent task should be rejected"
    );
}

// ─── Reputation update after initial assignment ─────────────────────

#[test]
fn test_reputation_can_be_updated() {
    let (env, admin, client) = setup();
    let g = Address::generate(&env);

    client.add_guardian(&admin, &g);
    client.set_reputation(&admin, &g, &100u64);
    assert_eq!(client.get_reputation(&g), Some(100));

    // Update reputation
    client.set_reputation(&admin, &g, &500u64);
    assert_eq!(client.get_reputation(&g), Some(500));
    assert_eq!(client.calculate_voting_power(&g), Some(500));
}
