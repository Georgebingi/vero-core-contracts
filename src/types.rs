use soroban_sdk::{contracterror, contracttype, Address};

#[contracttype]
#[derive(Clone)]
pub struct Task {
    pub id: u64,
    pub votes: u32,
    pub is_done: bool,
}

/// Represents an active reward stream initiated via the Drips protocol
/// after a task has been verified by guardian consensus.
#[contracttype]
#[derive(Clone)]
pub struct RewardStream {
    pub task_id: u64,
    pub contributor: Address,
    pub drips_contract: Address,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Guardian(Address),
    Task(u64),
    Voted(u64, Address), // (task_id, guardian)
    Admin,
    DripsAddress,
    RewardStream(u64), // keyed by task_id
    Lock,              // re-entrancy mutex
}

#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractError {
    NotAuthorized = 1,
    DuplicateVote = 2,
    TaskNotVerified = 3,
    StreamAlreadyActive = 4,
    DripsCallFailed = 5,
    Locked = 6,
}
