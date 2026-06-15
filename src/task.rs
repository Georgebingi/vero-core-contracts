use soroban_sdk::{Address, Env};

use crate::types::{ContractError, DataKey, Task};

pub fn register_task(env: &Env, admin: Address, task_id: u64) -> Result<(), ContractError> {
    admin.require_auth();

    let key = DataKey::Task(task_id);
    if env.storage().instance().has(&key) {
        return Err(ContractError::NotAuthorized);
    }

    let task = Task {
        id: task_id,
        votes: 0,
        is_done: false,
        total_weight_accrued: 0,
    };
    env.storage().instance().set(&key, &task);
    Ok(())
}

pub fn get_task(env: &Env, task_id: u64) -> Option<Task> {
    env.storage()
        .instance()
        .get(&DataKey::Task(task_id))
}
