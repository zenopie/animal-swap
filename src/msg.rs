use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Uint128};

use crate::state::State;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub contract_manager: String,
    pub token_erth_contract: String,
    pub token_erth_hash: String,
    pub token_b_contract: String,
    pub token_b_hash: String,
    pub registration_contract: String,
    pub registration_hash: String,
    pub token_b_symbol: String,
    pub lp_token_decimals: u8,
    pub lp_token_hash: String,
    pub lp_token_code_id: u64,
    pub lp_staking_contract: String,
    pub lp_staking_hash: String,
    pub protocol_fee: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Snip20InstantiateMsg {
    pub name: String,
    pub admin: Option<String>,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Option<Vec<InitialBalance>>,
    pub prng_seed: Binary,
    pub config: Option<InitConfig>,
    pub supported_denoms: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InitialBalance {
    pub address: String,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InitConfig {
    pub public_total_supply: Option<bool>,
    pub enable_deposit: Option<bool>,
    pub enable_redeem: Option<bool>,
    pub enable_mint: Option<bool>,
    pub enable_burn: Option<bool>,
    pub can_modify_denoms: Option<bool>,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddLiquidity {
        amount_erth: Uint128,
        amount_b: Uint128,
    },
    UpdateState {
        key: String,
        value: String,
    },
    Receive {
        sender: String,
        from: String,
        amount: Uint128,
        msg: Binary,
        memo: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    UnbondLiquidity {},
    Swap { 
        min_received: Option<Uint128> 
    },
    ErthBuybackSwap {},
    AnmlBuybackSwap {},
}

/// This struct represents the message to send to the other contract.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SendMessage {
    BurnAnml {},
    BurnErth {},
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {
    Migrate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    QueryState {},
    QueryDeposit { address: String },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct QueryStateResponse {
    pub state: State,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct QuerySwapResponse {
    pub protocol_fee_amount: Uint128,
    pub output_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct UnclaimedDepositResponse {
    pub unclaimed_deposit: Uint128,
}

