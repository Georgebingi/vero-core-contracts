use soroban_sdk::{contracterror, contracttype, Address};

#[contracttype]
#[derive(Clone)]
pub struct Task {
    pub id: u64,
    pub votes: u32,
    pub is_done: bool,
    /// Cumulative reputation weight accrued from all guardian votes.
    /// Consensus is reached when this meets or exceeds the weight threshold.
    pub total_weight_accrued: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Guardian(Address),
    Task(u64),
    Voted(u64, Address), // (task_id, guardian)
    Admin,
    /// Maps a guardian address to their u64 reputation score.
    Reputation(Address),
    /// The minimum cumulative weight required to resolve a task.
    WeightThreshold,
}

#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractError {
    NotAuthorized = 1,
    DuplicateVote = 2,
    /// The guardian has no reputation score assigned.
    NoReputationScore = 3,
    /// A zero-weight vote is not allowed.
    ZeroWeightVote = 4,
    /// Arithmetic overflow when accumulating vote weight.
    WeightOverflow = 5,
}
