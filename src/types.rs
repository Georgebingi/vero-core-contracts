use soroban_sdk::{contracterror, contracttype, Address, Map, Vec};

#[contracttype]
#[derive(Clone)]
pub struct Task {
    pub id: u64,
    pub votes: u32,
    pub is_done: bool,
    pub total_weight_accrued: u64,
    pub is_cancelled: bool,
    pub resolved_at: u64,
}

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
pub struct Snapshot {
    pub timestamp: u64,
    pub paused: bool,
    pub failure_count: u32,
    pub weight_threshold: u64,
    pub admin: Option<Address>,
    pub vault_address: Option<Address>,
    pub drips_address: Option<Address>,
    pub guardians: Map<Address, bool>,
    pub reputations: Map<Address, u64>,
    pub tasks: Map<u64, Task>,
    pub votes: Map<(u64, Address), bool>,
    pub reward_streams: Map<u64, RewardStream>,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Guardian(Address),
    Reputation(Address),
    WeightThreshold,
    Task(u64),
    Voted(u64, Address),
    Admin,
    DripsAddress,
    VaultAddress,
    RewardStream(u64),
    TokenAddress,
    LockThreshold,
    LockedBalance(Address),
    Lock,
    FailureCount,
    Paused,
    AllGuardians,
    AllTasks,
    AllVotes,
    AllRewardStreams,
    Snapshot(u64),
    AllSnapshots,
    ActiveTask(u64),
    ArchivedTask(u64),
    Initialized,
    ActiveTask(u64),
    ArchivedTask(u64),
}

/// Every public write operation exposed by VeroContract.
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    RegisterTask = 0,
    Vote = 1,
    AddGuardian = 2,
    SetReputation = 3,
    LockTokens = 4,
    UnlockTokens = 5,
    ResignGuardian = 6,
    SetWeightThreshold = 7,
    StartRewardStream = 8,
    TogglePause = 9,
    RecordFailure = 10,
    ResetCircuitBreaker = 11,
    UpgradeContract = 12,
    /// `record_snapshot` — records a state snapshot.
    RecordSnapshot = 13,
}

#[contracterror]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractError {
    NotAuthorized = 1,
    DuplicateVote = 2,
    TaskNotVerified = 3,
    StreamAlreadyActive = 4,
    DripsCallFailed = 5,
    AlreadyInitialized = 6,
    NotInitialized = 7,
    NoReputationScore = 8,
    ZeroWeightVote = 9,
    WeightOverflow = 10,
    InsufficientLockedBalance = 11,
    StillGuardian = 12,
    NotGuardian = 13,
    Locked = 14,
    ContractPaused = 15,
    EscrowUnavailable = 16,
    TaskCancelled = 17,
    TaskNotFound = 18,
    BatchTooLarge = 19,
    TaskAlreadyArchived = 20,
    TaskNotStale = 21,
    SnapshotNotFound = 22,
}
