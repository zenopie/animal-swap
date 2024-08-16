use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};

use secret_toolkit_storage::{Keymap, Item};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub contract_manager: Addr,
    pub token_erth_contract: Addr,
    pub token_erth_hash: String,
    pub token_b_contract: Addr,
    pub token_b_hash: String,
    pub token_b_symbol: String,
    pub burn_contract: Addr,
    pub burn_hash: String,
    pub registration_contract: Addr,
    pub registration_hash: String,
    pub lp_token_contract: Addr,
    pub lp_token_hash: String,
    pub lp_token_code_id: u64,
    pub lp_staking_contract: Addr,
    pub lp_staking_hash: String,
    pub lp_staking_code_id: u64,
    pub token_erth_reserve: Uint128,
    pub token_b_reserve: Uint128,
    pub total_shares: Uint128,
    pub protocol_fee: Uint128,
}

pub static STATE: Item<State> = Item::new(b"state");

pub const DEPOSITS: Keymap<Addr, Uint128> = Keymap::new(b"deposits");





