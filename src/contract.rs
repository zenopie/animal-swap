use cosmwasm_std::{
    entry_point, to_binary, from_binary, Binary, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Addr, Uint128, CosmosMsg,
    WasmMsg, SubMsg, Reply, SubMsgResponse,
};
use secret_toolkit::snip20;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, QueryMsg, QueryStateResponse, QuerySwapResponse,
    ReceiveMsg, UnclaimedDepositResponse, MigrateMsg, HopDetails,
    Snip20InstantiateMsg, InitConfig, SendMessage,
};
use crate::state::{STATE, State, DEPOSITS};

const INSTANTIATE_LP_TOKEN_REPLY_ID: u64 = 0;
const ERTH_DAO: &str = "secret1hxrvx0v0zvqgmpuzspdg5j8rrxpjgyjql3w9gh";
const CONTRACT_VERSION: &str = "v0.0.22";

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // Validate and convert strings to Addr
    let contract_manager = deps.api.addr_validate(&msg.contract_manager)?;
    let token_erth_contract = deps.api.addr_validate(&msg.token_erth_contract)?;
    let token_b_contract = deps.api.addr_validate(&msg.token_b_contract)?;
    let registration_contract_addr = deps.api.addr_validate(&msg.registration_contract)?;
    let lp_staking_contract_addr = deps.api.addr_validate(&msg.lp_staking_contract)?;


    let lp_token_name = format!("ERTH-{} Animal Swap LP Token", msg.token_b_symbol);
    let lp_token_symbol = format!("{}LP", msg.token_b_symbol);

    let init_config = InitConfig {
        public_total_supply: Some(true),
        enable_deposit: Some(false),
        enable_redeem: Some(false),
        enable_mint: Some(true),
        enable_burn: Some(true),
        can_modify_denoms: Some(false),
    };

    // Construct the SNIP-20 instantiation message
    let lp_token_instantiate_msg = Snip20InstantiateMsg {
        name: lp_token_name.clone(),
        admin: Some(env.contract.address.to_string()), // Use the validated address
        symbol: lp_token_symbol,
        decimals: 6,
        initial_balances: None,
        prng_seed: to_binary(&env.block.time.seconds())?,
        config: Some(init_config),
        supported_denoms: None,
    };

    // Instantiate the LP token contract
    let lp_token_msg = WasmMsg::Instantiate {
        admin: Some(ERTH_DAO.to_string()), 
        code_id: msg.lp_token_code_id,
        code_hash: msg.lp_token_hash.clone(),
        msg: to_binary(&lp_token_instantiate_msg)?,
        funds: vec![],
        label: format!("{} {}", lp_token_name, CONTRACT_VERSION),
    };

    // Submessage for LP token instantiation
    let sub_msg_lp = SubMsg::reply_on_success(CosmosMsg::Wasm(lp_token_msg), INSTANTIATE_LP_TOKEN_REPLY_ID);


    // Initialize the state with placeholder addresses for the LP token and staking contracts
    let state = State {
        contract_manager: contract_manager.clone(),
        token_erth_contract: token_erth_contract.clone(),
        token_erth_hash: msg.token_erth_hash.clone(),
        token_b_contract: token_b_contract.clone(),
        token_b_hash: msg.token_b_hash.clone(),
        token_b_symbol: msg.token_b_symbol,
        registration_contract: registration_contract_addr.clone(),
        registration_hash: msg.registration_hash,
        lp_token_contract: Addr::unchecked(""), // Placeholder
        lp_token_hash: msg.lp_token_hash.clone(),
        lp_token_code_id: msg.lp_token_code_id,
        lp_staking_contract: lp_staking_contract_addr, // Placeholder
        lp_staking_hash: msg.lp_staking_hash.clone(),
        token_erth_reserve: Uint128::zero(),
        token_b_reserve: Uint128::zero(),
        total_shares: Uint128::zero(),
        protocol_fee: msg.protocol_fee,
    };

    // Save the initial state
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_submessage(sub_msg_lp)
        .add_attribute("action", "instantiate"))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, StdError> {
    match msg {
        ExecuteMsg::AddLiquidity { amount_erth, amount_b } =>
            execute_add_liquidity(deps, env, info, amount_erth, amount_b),
        ExecuteMsg::UpdateState { key, value } => execute_update_state(deps, env, info, key, value),
        ExecuteMsg::Receive { sender, from, amount, msg, memo: _ } =>
            execute_receive(deps, env, info, sender, from, amount, msg),
    }
}


pub fn execute_add_liquidity(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount_erth: Uint128,
    amount_b: Uint128,
) -> Result<Response, StdError> {
    let mut state = STATE.load(deps.storage)?;

    let (shares, adjusted_amount_erth, adjusted_amount_b) = if state.total_shares.is_zero() {
        // Initial liquidity: use the provided amounts directly and set the total shares to the sum
        let shares = amount_erth + amount_b;
        (shares, amount_erth, amount_b)
    } else {
        // Subsequent liquidity
        let share_erth = amount_erth * state.total_shares / state.token_erth_reserve;
        let share_b = amount_b * state.total_shares / state.token_b_reserve;
        let shares = share_erth.min(share_b);

        // Adjust amounts based on the limiting factor
        let adjusted_amount_erth = (shares * state.token_erth_reserve) / state.total_shares;
        let adjusted_amount_b = (shares * state.token_b_reserve) / state.total_shares;

        (shares, adjusted_amount_erth, adjusted_amount_b)
    };

    // Calculate the excess amount of the token that exceeds the required ratio
    let (excess_token, excess_amount) = if amount_erth > adjusted_amount_erth {
        (state.token_erth_contract.clone(), amount_erth - adjusted_amount_erth)
    } else {
        (state.token_b_contract.clone(), amount_b - adjusted_amount_b)
    };

    let mut messages = vec![];

    // Create messages for transferring tokens from the user to the contract using allowances
    let transfer_erth_msg = snip20::HandleMsg::TransferFrom {
        owner: info.sender.clone().to_string(),
        recipient: env.contract.address.clone().to_string(),
        amount: adjusted_amount_erth,
        padding: None,
        memo: None,
    };
    let transfer_b_msg = snip20::HandleMsg::TransferFrom {
        owner: info.sender.clone().to_string(),
        recipient: env.contract.address.clone().to_string(),
        amount: adjusted_amount_b,
        padding: None,
        memo: None,
    };

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_erth_contract.to_string(),
        code_hash: state.token_erth_hash.clone(),
        msg: to_binary(&transfer_erth_msg)?,
        funds: vec![],
    }));
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_b_contract.to_string(),
        code_hash: state.token_b_hash.clone(),
        msg: to_binary(&transfer_b_msg)?,
        funds: vec![],
    }));

    // Refund the excess token if any
    if excess_amount > Uint128::from(2u32) {
        let refund_msg = snip20::HandleMsg::Transfer {
            recipient: info.sender.clone().to_string(),
            amount: excess_amount,
            padding: None,
            memo: None,
        };
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: excess_token.to_string(),
            code_hash: if excess_token == state.token_erth_contract {
                state.token_erth_hash.clone()
            } else {
                state.token_b_hash.clone()
            },
            msg: to_binary(&refund_msg)?,
            funds: vec![],
        }));
    }

    // Update reserves
    state.token_erth_reserve += adjusted_amount_erth;
    state.token_b_reserve += adjusted_amount_b;

    state.total_shares += shares;

    // Mint LP tokens
    let mint_lp_tokens_msg = snip20::HandleMsg::Mint {
        recipient: info.sender.clone().to_string(),
        amount: shares,
        memo: None,
        padding: None,
    };
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.lp_token_contract.to_string(),
        code_hash: state.lp_token_hash.clone(),
        msg: to_binary(&mint_lp_tokens_msg)?,
        funds: vec![],
    }));

    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "add_liquidity")
        .add_attribute("from", info.sender)
        .add_attribute("shares", shares.to_string())
        .add_attribute("adjusted_amount_erth", adjusted_amount_erth.to_string())
        .add_attribute("adjusted_amount_b", adjusted_amount_b.to_string()))
}


pub fn execute_update_state(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    key: String,
    value: String,
) -> Result<Response, StdError> {
    let mut state = STATE.load(deps.storage)?;

    if info.sender != state.contract_manager {
        return Err(StdError::generic_err("unauthorized"));
    }

    match key.as_str() {
        "contract_manager" => {
            state.contract_manager = deps.api.addr_validate(&value)?;
        }
        "protocol_fee" => {
            let protocol_fee: Uint128 = value.parse().map_err(|_| StdError::generic_err("Invalid protocol_fee"))?;
            state.protocol_fee = protocol_fee;
        }
        "token_erth_hash" => {
            state.token_erth_hash = value.clone();
        }
        "token_b_hash" => {
            state.token_b_hash = value.clone();
        }
        "lp_token_hash" => {
            state.lp_token_hash = value.clone();
        }
        "lp_staking_hash" => {
            state.lp_staking_hash = value.clone();
        }
        "lp_staking_contract" => {
            state.lp_staking_contract = deps.api.addr_validate(&value)?;
        }
        "registration_contract" => {
            state.registration_contract = deps.api.addr_validate(&value)?;
        }
        "registration_hash" => {
            state.registration_hash = value.clone();
        }
        _ => return Err(StdError::generic_err("Invalid state key")),
    }

    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "update_state")
        .add_attribute("key", key)
        .add_attribute("value", value))
}



pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    from: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, StdError> {
    let msg: ReceiveMsg = from_binary(&msg)?;

    let _sender_addr = deps.api.addr_validate(&sender)?;
    let from_addr = deps.api.addr_validate(&from)?;

    match msg {
        ReceiveMsg::Swap {min_received, hop, user} => receive_swap(deps, env, info, from_addr, amount, min_received, hop, user),
        ReceiveMsg::UnbondLiquidity {} => recieve_unbond_liquidity(deps, env, info, from_addr, amount),
        ReceiveMsg::ErthBuybackSwap {} => receive_erth_buyback_swap(deps, info, amount),
        ReceiveMsg::AnmlBuybackSwap {} => receive_anml_buyback_swap(deps, info, amount),

    }
}



fn receive_swap(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    mut from: Addr,
    amount: Uint128,
    min_received: Option<Uint128>,
    hop: Option<HopDetails>,
    user: Option<Addr>,
) -> Result<Response, StdError> {
    // Load state
    let mut state = STATE.load(deps.storage)?;
    let input_amount = amount;
    let input_token = info.sender.clone();

    // Calculate the swap details and update reserves directly, including trade volume in ERTH
    let (protocol_fee_amount, output_amount, output_addr, output_hash, trade_volume_in_erth) =
        calculate_swap(&mut state, input_amount, &input_token)?;

    let mut messages = vec![];

    // Handle the protocol fee in ERTH
    let buyback_msg = snip20::HandleMsg::Send {
        recipient: state.lp_staking_contract.to_string(),
        recipient_code_hash: Some(state.lp_staking_hash.clone()),
        amount: protocol_fee_amount,
        msg: Some(to_binary(&SendMessage::BurnErth {
            trade_volume: trade_volume_in_erth, 
            pool_liquidity: state.token_erth_reserve * Uint128::from(2u32),
            total_shares: state.total_shares.clone(),
        })?),
        memo: None,
        padding: None,
    };

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_erth_contract.to_string(),
        code_hash: state.token_erth_hash.clone(),
        msg: to_binary(&buyback_msg)?,
        funds: vec![],
    }));

    // Check if hop details are provided
    if let Some(hop_details) = hop {
        // Try to validate the hop contract address
        let hop_addr = deps.api.addr_validate(&hop_details.contract)?;

        let hop_msg = snip20::HandleMsg::Send {
            recipient: hop_addr.to_string(),
            recipient_code_hash: Some(hop_details.hash.clone()),
            amount: output_amount,
            msg: Some(to_binary(&SendMessage::Swap {
                min_received: min_received.clone(),
                user: from.clone(),
            })?),
            memo: None,
            padding: None,
        };

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: output_addr.to_string(),
            code_hash: output_hash.clone(),
            msg: to_binary(&hop_msg)?,
            funds: vec![],
        }));

    } else {
        // Check against minimum received amount
        if let Some(min) = min_received {
            if output_amount < min {
                return Err(StdError::generic_err("Output amount is less than the minimum received amount"));
            }
        }

        // Check for user parameter in the case of the second part of the hop
        if let Some(user_addr) = user {
            from = user_addr;
        }
        

        // Create transfer message if `from` is provided
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: output_addr.to_string(),
            code_hash: output_hash,
            msg: to_binary(&snip20::HandleMsg::Transfer {
                recipient: from.to_string(),
                amount: output_amount,
                padding: None,
                memo: None,
            })?,
            funds: vec![],
        }));
    }

    // Save the updated state
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "swap")
        .add_attribute("from", from.to_string())
        .add_attribute("input_amount", amount.to_string())
        .add_attribute("output_amount", output_amount.to_string())
        .add_attribute("protocol_fee_amount", protocol_fee_amount.to_string())
        .add_attribute("trade_volume_in_erth", trade_volume_in_erth.to_string()))  // Add trade volume attribute
}



fn calculate_swap(
    state: &mut State,  // Mutably borrow the state so we can update reserves
    input_amount: Uint128,
    input_token: &Addr,
) -> Result<(Uint128, Uint128, Addr, String, Uint128), StdError> {
    // Calculate protocol fee in the input token
    let mut protocol_fee_amount = input_amount * state.protocol_fee / Uint128::from(10000u128);
    let amount_after_protocol_fee = input_amount - protocol_fee_amount;

    // Extract all necessary details from the state
    let (input_reserve, output_reserve, output_addr, output_hash, trade_volume_in_erth) = if input_token == &state.token_erth_contract {
        (
            state.token_erth_reserve,
            state.token_b_reserve,
            state.token_b_contract.clone(),
            state.token_b_hash.clone(),
            input_amount,  // Trade volume is the input amount in ERTH
        )
    } else if input_token == &state.token_b_contract {
        (
            state.token_b_reserve,
            state.token_erth_reserve,
            state.token_erth_contract.clone(),
            state.token_erth_hash.clone(),
            // Convert input token volume to ERTH using reserve ratio
            (input_amount * state.token_erth_reserve) / state.token_b_reserve,
        )
    } else {
        return Err(StdError::generic_err("Invalid input token"));
    };

    // Calculate the output amount using the constant product formula
    let output_amount = (amount_after_protocol_fee * output_reserve)
        / (input_reserve + amount_after_protocol_fee);

    // Check if the liquidity is enough
    if output_amount > output_reserve {
        return Err(StdError::generic_err("Insufficient liquidity in reserves"));
    }

    // Update the reserves based on the swap
    if input_token == &state.token_erth_contract {
        state.token_erth_reserve += amount_after_protocol_fee; // Add input amount to ERTH reserve
        state.token_b_reserve -= output_amount; // Subtract output amount from token B reserve
    } else if input_token == &state.token_b_contract {
        state.token_b_reserve += amount_after_protocol_fee; // Add to token B reserve after protocol fee is deducted
        state.token_erth_reserve -= output_amount;          // Subtract from ERTH reserve (as we are sending this amount)

        // Perform feeless swap to convert protocol fee to ERTH
        let protocol_fee_in_erth = calculate_feeless_swap(&state, protocol_fee_amount, &state.token_b_contract)?;

        //update reserves
        state.token_b_reserve += protocol_fee_amount;
        state.token_erth_reserve -= protocol_fee_in_erth;

        // The `protocol_fee_amount` now represents the amount in ERTH
        protocol_fee_amount = protocol_fee_in_erth;
    }

    // Return the result including the protocol fee (now in ERTH), output amount, and other details
    Ok((
        protocol_fee_amount, // Protocol fee in ERTH
        output_amount,
        output_addr,
        output_hash,
        trade_volume_in_erth,
    ))
}






fn receive_erth_buyback_swap(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, StdError> {

    let mut state = STATE.load(deps.storage)?;
    let input_token = info.sender.clone();

    if input_token != state.token_b_contract {
        return Err(StdError::generic_err("invalid input token for erth buyback contract"));
    }

    // Calculate the swap details without fees
    let output_amount = calculate_feeless_swap(&mut state, amount, &input_token)?;

    // Update reserves
    state.token_b_reserve += amount;
    state.token_erth_reserve -= output_amount;

    // Save state
    STATE.save(deps.storage, &state)?;

    // Create a Send message to send the output amount back to the buyback contract for burning
    let buyback_msg = snip20::HandleMsg::Send {
        recipient: state.lp_staking_contract.to_string(),
        recipient_code_hash: Some(state.lp_staking_hash.clone()),
        amount: output_amount,
        msg: Some(to_binary(&SendMessage::BurnErth {
            trade_volume: output_amount,
            pool_liquidity: state.token_erth_reserve * Uint128::from(2u32),
            total_shares: state.total_shares.clone(),
        })?),
        memo: None,
        padding: None,
    };

    // Create the message to execute the Send
    let send_message = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_erth_contract.to_string(),
        code_hash: state.token_erth_hash.clone(),
        msg: to_binary(&buyback_msg)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(send_message)
        .add_attribute("action", "buyback_swap")
        .add_attribute("input_amount", amount.to_string())
        .add_attribute("output_amount", output_amount.to_string()))
}

// function only used in the ANML-ERTH pair for 1/second ERTH->ANML buyback and burn
fn receive_anml_buyback_swap(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, StdError> {

    let mut state = STATE.load(deps.storage)?;
    let input_token = info.sender.clone();

    if input_token != state.token_erth_contract {
        return Err(StdError::generic_err("invalid input token for anml buyback contract"));
    }

    // Calculate the swap details without fees
    let output_amount = calculate_feeless_swap(&mut state, amount, &input_token)?;

    // Update reserves
    state.token_erth_reserve += amount;
    state.token_b_reserve -= output_amount;

    // Save state
    STATE.save(deps.storage, &state)?;

    // Create a Send message to send the output amount back to the buyback contract for burning
    let buyback_msg = snip20::HandleMsg::Send {
        recipient: state.lp_staking_contract.to_string(),
        recipient_code_hash: Some(state.lp_staking_hash.clone()),
        amount: output_amount,
        msg: Some(to_binary(&SendMessage::BurnAnml {
            trade_volume: amount,
            pool_liquidity: state.token_erth_reserve * Uint128::from(2u32),
            total_shares: state.total_shares.clone(),
        })?),
        memo: None,
        padding: None,
    };

    // Create the message to execute the Send
    let send_message = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_b_contract.to_string(),
        code_hash: state.token_b_hash.clone(),
        msg: to_binary(&buyback_msg)?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(send_message)
        .add_attribute("action", "buyback_swap")
        .add_attribute("input_amount", amount.to_string())
        .add_attribute("output_amount", output_amount.to_string()))
}

fn calculate_feeless_swap(
    state: &State, 
    input_amount: Uint128,
    input_token: &Addr,
) -> Result<Uint128, StdError> {
    // Extract the reserves immutably before mutating state
    let (input_reserve, output_reserve) = if input_token == &state.token_b_contract {
        (state.token_b_reserve, state.token_erth_reserve)
    } else if input_token == &state.token_erth_contract {
        (state.token_erth_reserve, state.token_b_reserve)
    } else {
        return Err(StdError::generic_err("Invalid input token for feeless swap"));
    };

    // Calculate the output amount using the constant product formula
    let output_amount = (input_amount * output_reserve)
        / (input_reserve + input_amount);

    // Check if there is enough liquidity in the reserves
    if output_amount > output_reserve {
        return Err(StdError::generic_err(
            "Insufficient liquidity in reserves for feeless swap",
        ));
    }

    // Return the calculated output amount (which is in ERTH)
    Ok(output_amount)
}



pub fn recieve_unbond_liquidity(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    from: Addr,
    lp_token_amount: Uint128,
) -> Result<Response, StdError> {
    // Load the state
    let mut state = STATE.load(deps.storage)?;

    if info.sender != state.lp_token_contract {
        return Err(StdError::generic_err("Invalid LP token"));
    }

    // Calculate the amount of ERTH and B tokens to return
    let amount_erth = (lp_token_amount * state.token_erth_reserve) / state.total_shares;
    let amount_b = (lp_token_amount * state.token_b_reserve) / state.total_shares;

    // Update the state reserves and total shares
    state.token_erth_reserve -= amount_erth;
    state.token_b_reserve -= amount_b;

    // Adjust total shares based on the unbonding amounts
    state.total_shares -= lp_token_amount;

    STATE.save(deps.storage, &state)?;

    let mut messages = vec![];

    // Create message to burn the LP tokens
    let burn_lp_msg = snip20::HandleMsg::Burn {
        amount: lp_token_amount,
        memo: None,
        padding: None,
    };

    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.lp_token_contract.to_string(),
        code_hash: state.lp_token_hash.clone(),
        msg: to_binary(&burn_lp_msg)?,
        funds: vec![],
    }));

    // Transfer the unbonded tokens to the user
    let transfer_erth_msg = snip20::HandleMsg::Transfer {
        recipient: from.clone().to_string(),
        amount: amount_erth,
        padding: None,
        memo: None,
    };
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_erth_contract.to_string(),
        code_hash: state.token_erth_hash.clone(),
        msg: to_binary(&transfer_erth_msg)?,
        funds: vec![],
    }));

    let transfer_b_msg = snip20::HandleMsg::Transfer {
        recipient: from.clone().to_string(),
        amount: amount_b,
        padding: None,
        memo: None,
    };
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_b_contract.to_string(),
        code_hash: state.token_b_hash.clone(),
        msg: to_binary(&transfer_b_msg)?,
        funds: vec![],
    }));


    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "unbond_liquidity")
        .add_attribute("from", from)
        .add_attribute("erth_token_amount", amount_erth.to_string())
        .add_attribute("token_b_amount", amount_b.to_string())
        .add_attribute("lp_token_amount", lp_token_amount.to_string()))
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> StdResult<Response> {
    match msg.id {
        INSTANTIATE_LP_TOKEN_REPLY_ID => handle_instantiate_lp_token_reply(deps, env, msg),
        _ => Err(StdError::generic_err("Unknown reply ID")),
    }
}

fn handle_instantiate_lp_token_reply(
    deps: DepsMut,
    env: Env,
    msg: Reply,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;

    // Extract the SubMsgExecutionResponse from the reply
    let res: SubMsgResponse = msg.result.unwrap();

    // Find the event that contains the contract address
    let contract_address_event = res
        .events
        .iter()
        .find(|event| event.ty == "instantiate");

    // Ensure we found the instantiate event
    let contract_address_event = match contract_address_event {
        Some(event) => event,
        None => return Err(StdError::generic_err("Failed to find instantiate event")),
    };

    // Find the attribute that contains the contract address
    let contract_address_attr = contract_address_event
        .attributes
        .iter()
        .find(|attr| attr.key == "contract_address");

    // Ensure we found the contract address attribute
    let contract_address = match contract_address_attr {
        Some(attr) => &attr.value,
        None => return Err(StdError::generic_err("Failed to find contract address")),
    };

    // Validate the contract address
    let lp_token_contract_addr = deps.api.addr_validate(contract_address)?;

    // Update the state with the LP token contract address
    state.lp_token_contract = lp_token_contract_addr.clone();
    STATE.save(deps.storage, &state)?;

    // Register this contract as a receiver for the LP token
    let register_lp_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: lp_token_contract_addr.to_string(),
        code_hash: state.lp_token_hash.clone(),
        msg: to_binary(&snip20::HandleMsg::RegisterReceive {
            code_hash: env.contract.code_hash.clone(),
            padding: None,  // Optional padding
        })?,
        funds: vec![],
    });

    // Register the contract as a receiver for the ERTH token
    let register_erth_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_erth_contract.to_string(),
        code_hash: state.token_erth_hash.clone(),
        msg: to_binary(&snip20::HandleMsg::RegisterReceive {
            code_hash: env.contract.code_hash.clone(),
            padding: None,
        })?,
        funds: vec![],
    });

    // Register the contract as a receiver for the B token
    let register_b_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: state.token_b_contract.to_string(),
        code_hash: state.token_b_hash.clone(),
        msg: to_binary(&snip20::HandleMsg::RegisterReceive {
            code_hash: env.contract.code_hash.clone(),
            padding: None,
        })?,
        funds: vec![],
    });


    Ok(Response::new()
        .add_message(register_lp_msg) // Add the registration message for the LP token
        .add_message(register_erth_msg)
        .add_message(register_b_msg)
        .add_attribute("action", "instantiate_lp_token")
        .add_attribute("lp_token_contract", lp_token_contract_addr.to_string()))
}



#[entry_point]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> StdResult<Response> {
    match msg {
        MigrateMsg::Migrate {} => {

            // Load the state
            let state = STATE.load(deps.storage)?;

            // Register this contract as a receiver for ERTH
            let register_erth_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.token_erth_contract.to_string(),
                code_hash: state.token_erth_hash,
                msg: to_binary(&snip20::HandleMsg::RegisterReceive {
                    code_hash: env.contract.code_hash.clone(),
                    padding: None,  // Optional padding
                })?,
                funds: vec![],
            });

            let register_b_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.token_b_contract.to_string(),
                code_hash: state.token_b_hash,
                msg: to_binary(&snip20::HandleMsg::RegisterReceive {
                    code_hash: env.contract.code_hash.clone(),
                    padding: None,  // Optional padding
                })?,
                funds: vec![],
            });

            let register_lp_msg = CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: state.lp_token_contract.to_string(),
                code_hash: state.lp_token_hash,
                msg: to_binary(&snip20::HandleMsg::RegisterReceive {
                    code_hash: env.contract.code_hash.clone(),
                    padding: None,  // Optional padding
                })?,
                funds: vec![],
            });

            Ok(Response::new()
                .add_message(register_erth_msg)
                .add_message(register_b_msg)
                .add_message(register_lp_msg)
                .add_attribute("action", "migrate"))
        }
    }
}



#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::QueryState {} => to_binary(&query_state(deps)?),
        QueryMsg::QueryDeposit { address } => {
            let address = deps.api.addr_validate(&address)?;
            to_binary(&query_deposit(deps, address)?)
        },
    }
}

pub fn query_swap(
    deps: Deps,
    input_amount: Uint128,
    input_token: Addr,
) -> StdResult<QuerySwapResponse> {
    // Load state
    let mut state = STATE.load(deps.storage)?;

    // Calculate the swap details without creating messages
    let (protocol_fee_amount, output_amount, _, _, _) =
        calculate_swap(&mut state, input_amount, &input_token)?;

    Ok(QuerySwapResponse {
        protocol_fee_amount,
        output_amount,
    })
}

fn query_state(deps: Deps) -> StdResult<QueryStateResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(QueryStateResponse { state })
}

pub fn query_deposit(deps: Deps, address: Addr) -> StdResult<UnclaimedDepositResponse> {
    // Query deposit amount
    let unclaimed_deposit = DEPOSITS
        .get(deps.storage, &address)
        .unwrap_or_else(Uint128::zero);

    let unclaimed_deposit_response = UnclaimedDepositResponse {
        unclaimed_deposit,
    };

    Ok(unclaimed_deposit_response)
}
