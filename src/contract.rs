use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdError, StdResult,
};

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, CONFIG},
};

pub const CONFIRM_ORDER_REPLY_ID: u64 = 1;

const CONTRACT_NAME: &str = "crates.io/cw-atomic-swap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate specified owner or use sender.
    let owner = deps
        .api
        .addr_validate(&msg.owner.unwrap_or(info.sender.to_string()))?;

    CONFIG.save(deps.storage, &Config { owner })?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    use ExecuteMsg::*;
    match msg {
        UpdateConfig { new_owner } => execute::update_config(deps, env, &info.sender, new_owner),
        CreateSwapOrder {
            coin_in,
            coin_out,
            taker,
            timeout,
        } => execute::create_swap_order(deps, env, info, coin_in, coin_out, taker, timeout),
        AcceptSwapOrder { order_id, maker } => {
            execute::accept_swap_order(deps, info, env, order_id, maker)
        }
        ConfirmSwapOrder { order_id, maker } => {
            execute::confirm_swap_order(deps, info, env, order_id, maker)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    use QueryMsg::*;
    match msg {
        Config {} => to_json_binary(&query::get_config(deps)?),
        AllSwapOrders {} => to_json_binary(&query::get_all_swap_orders(deps, env)?),
        SwapOrdersByMaker { maker } => {
            to_json_binary(&query::get_orders_by_maker(deps, env, maker)?)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    deps.api
        .debug("Failed execution, entered in reply entry point");
    match msg.id {
        CONFIRM_ORDER_REPLY_ID => reply::reply_confirm_order(deps),
        _ => Err(StdError::generic_err(format!("received unkown reply id: {}", msg.id)).into()),
    }
}

pub mod execute {
    use cosmwasm_std::{ensure, Addr, BankMsg, Coin, CosmosMsg, SubMsg};
    use osmosis_std::types::cosmos::authz::v1beta1::MsgExec;

    use crate::state::{next_id, OrderPointer, OrderStatus, SwapOrder, ORDER_POINTER, SWAP_ORDERS};
    use crate::utils::{
        check_correct_coins, create_authz_encoded_message, validate_coins_number,
        validate_different_denoms, validate_native_denom, validate_status_and_expiration,
    };

    use super::*;

    /// Allows to update the atomic swap market configuration.
    pub fn update_config(
        deps: DepsMut,
        _env: Env,
        sender: &Addr,
        new_owner: String,
    ) -> Result<Response, ContractError> {
        let mut config = CONFIG.load(deps.storage)?;
        ensure!(config.owner == sender, ContractError::Unauthorized);
        config.owner = deps.api.addr_validate(&new_owner)?;

        CONFIG.save(deps.storage, &config)?;
        Ok(Response::new()
            .add_attribute("action", "update_config")
            .add_attribute("new_owner", new_owner))
    }

    /// Create a new atomic swap order.
    ///
    /// # Errors
    ///
    ///- `coin_in` and `coin_out` are the same.
    /// - coins to swap are not native.
    /// - coins sent to the contract along with the message.
    pub fn create_swap_order(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        coin_in: Coin,
        coin_out: Coin,
        taker: Option<String>,
        timeout: u64,
    ) -> Result<Response, ContractError> {
        validate_different_denoms(&coin_in.denom, &coin_out.denom)?;
        validate_native_denom(&coin_in.denom)?;
        validate_native_denom(&coin_out.denom)?;
        validate_coins_number(&info.funds, 0)?;

        let taker = taker
            .as_ref()
            .map(|addr| deps.api.addr_validate(addr))
            .transpose()?;

        let swap_order = SwapOrder {
            coin_in,
            coin_out,
            taker,
            timeout: env.block.time.plus_seconds(timeout).seconds(),
            status: OrderStatus::Open,
        };

        let order_id: u64 = next_id(deps.storage)?;
        SWAP_ORDERS.save(deps.storage, (&info.sender, order_id), &swap_order)?;

        Ok(Response::new()
            .add_attribute("action", "create_swap_order")
            .add_attribute("order_id", order_id.to_string())
            .add_attribute("maker", info.sender))
    }

    // Accept a swap order.
    //
    // # Errors
    //
    // - more than one coin is sent to the contract.
    // - sender is equal to matching order maker.
    // - selected order is not open or timed out.
    // - sent coin doesn't match maker wanted coin.
    // - sender is not the specified taker if specified.
    pub fn accept_swap_order(
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        order_id: u64,
        maker: String,
    ) -> Result<Response, ContractError> {
        deps.api.debug("Initiate acceptance of swap order");

        validate_coins_number(&info.funds, 1)?;

        // We don't care about validation because the address is used to match a key.
        let maker = Addr::unchecked(maker);
        if info.sender == maker {
            return Err(ContractError::SenderIsMaker {});
        }

        let mut order = SWAP_ORDERS.load(deps.storage, (&maker, order_id))?;

        validate_status_and_expiration(&order, OrderStatus::Open, env.block.time.seconds())?;
        check_correct_coins(&info.funds[0], &order.coin_out)?;

        // Check if the order is reserved and the sender is not the lucky one.
        if let Some(taker) = order.taker {
            if taker != info.sender {
                return Err(ContractError::Unauthorized {});
            }
        }

        order.taker = Some(info.sender.clone());
        order.status = OrderStatus::Accepted;

        SWAP_ORDERS.save(deps.storage, (&maker, order_id), &order)?;

        // Save a pointer used in `ConfirmSwapOrder`.
        // NOTE: the execution is atomic so it should
        // not be required. Left for security reason but should
        // be investigated.
        ORDER_POINTER.save(
            deps.storage,
            &OrderPointer {
                maker: maker.clone(),
                taker: info.sender,
                order_id,
            },
        )?;

        // Create encoded `x/authz` message to trigger `ConfirmSwapOrder`
        // on behalf of the order maker.
        let msg_exec = create_authz_encoded_message(
            env.contract.address.to_string(),
            order_id,
            maker.to_string(),
            order.coin_in,
        )?;
        let authz_msg: CosmosMsg = CosmosMsg::Stargate {
            type_url: MsgExec::TYPE_URL.to_string(),
            value: msg_exec.into(),
        };
        // TODO: reply on error to take action in case in which the maker
        // doesn't have required funds. What to do? Let's see in the future.
        let msg = SubMsg::reply_on_error(authz_msg, CONFIRM_ORDER_REPLY_ID);

        Ok(Response::new()
            .add_attribute("action", "accept_swap_order")
            .add_attribute("order_taker", order.taker.unwrap())
            .add_submessage(msg))
    }

    /// This function complete the execution of an order between a maker and a taker.
    /// The logic is executed via `x/authz` after receiving a `MsgExec` from this
    /// contract.
    //
    // # Errors
    //
    // - more than one coin is sent to the contract.
    // - sender is not the maker of the order.
    // - selected order is not open or timed out.
    // - sent coin doesn't match maker wanted coin.
    // - sender is not the specified taker if specified.
    pub fn confirm_swap_order(
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        order_id: u64,
        maker: String,
    ) -> Result<Response, ContractError> {
        deps.api.debug("Initiate confirm of swap order");

        validate_coins_number(&info.funds, 1)?;

        // Sender is the address of the user that granted this contract the
        // execution.
        if maker != info.sender {
            return Err(ContractError::Unauthorized {});
        }

        let mut order = SWAP_ORDERS.load(deps.storage, (&info.sender, order_id))?;

        // Return error if the order is expired or already matched.
        // NOTE: order should not be timeouted since the execution is atomic.
        if order.status != OrderStatus::Accepted || order.timeout < env.block.time.seconds() {
            return Err(ContractError::SwapOrderNotAvailable {
                status: order.status.to_string(),
                expiration: order.timeout,
            });
        }

        // Check if sent coins are the same of the selected order.
        check_correct_coins(&info.funds[0], &order.coin_in)?;

        order.status = OrderStatus::Confirmed;
        SWAP_ORDERS.save(deps.storage, (&info.sender, order_id), &order)?;
        ORDER_POINTER.remove(deps.storage);

        // Unwrapping is save because order is atomic.
        let taker = order.taker.unwrap();
        let msgs = vec![
            BankMsg::Send {
                to_address: maker,
                amount: vec![order.coin_out],
            },
            BankMsg::Send {
                to_address: taker.into_string(),
                amount: vec![order.coin_in],
            },
        ];

        Ok(Response::new()
            .add_messages(msgs)
            .add_attribute("action", "confirm_swap_order"))
    }
}

pub mod query {

    use cosmwasm_std::{Addr, Order};

    use crate::msg::{AllSwapOrdersResponse, SwapOrdersByMakerResponse};
    use crate::state::{SwapOrder, SWAP_ORDERS};

    use super::*;

    /// Returns the contract configuration.
    pub fn get_config(deps: Deps) -> StdResult<Config> {
        CONFIG.load(deps.storage)
    }

    /// Returns all active orders.
    pub fn get_all_swap_orders(deps: Deps, env: Env) -> StdResult<AllSwapOrdersResponse> {
        let current_time = env.block.time.seconds();
        let orders = SWAP_ORDERS
            .range(deps.storage, None, None, Order::Ascending)
            .filter_map(|item| {
                item.ok().and_then(|(addr, order)| {
                    if order.timeout > current_time {
                        Some(Ok((addr, order)))
                    } else {
                        None
                    }
                })
            })
            .collect::<StdResult<Vec<((Addr, u64), SwapOrder)>>>()?;
        Ok(AllSwapOrdersResponse { orders })
    }

    /// Returns the active orders associated with a creator.
    pub fn get_orders_by_maker(
        deps: Deps,
        env: Env,
        maker: String,
    ) -> StdResult<SwapOrdersByMakerResponse> {
        let current_time = env.block.time.seconds();
        let maker = Addr::unchecked(maker);

        let orders = SWAP_ORDERS
            .prefix(&maker)
            .range(deps.storage, None, None, Order::Ascending)
            .filter_map(|item| {
                item.ok().and_then(|(addr, order)| {
                    if order.timeout > current_time {
                        Some(Ok((addr, order)))
                    } else {
                        None
                    }
                })
            })
            .collect::<StdResult<Vec<(u64, SwapOrder)>>>()?;

        Ok(SwapOrdersByMakerResponse { orders })
    }
}

pub mod reply {
    use cosmwasm_std::{BankMsg, Coin, DepsMut, Response};

    use crate::error::ContractError;
    use crate::state::{OrderPointer, OrderStatus, ORDER_POINTER, SWAP_ORDERS};

    /// Handler the error during the execution of `ConfirmSwapOrder` sent via `x/authz`
    /// module.
    /// NOTE: currently all order status are not used but still included to
    /// easily extend functionalities in the future.
    pub fn reply_confirm_order(deps: DepsMut) -> Result<Response, ContractError> {
        let OrderPointer {
            order_id,
            maker,
            taker,
        } = ORDER_POINTER.load(deps.storage)?;

        let mut coin_out = Coin::default();
        SWAP_ORDERS.update(
            deps.storage,
            (&maker, order_id),
            |swap_order| match swap_order {
                Some(mut order) => {
                    order.status = OrderStatus::Failed;
                    coin_out = order.coin_out.clone();
                    Ok(order)
                }
                None => Err(ContractError::Unauthorized),
            },
        )?;
        ORDER_POINTER.remove(deps.storage);
        let refund_msg = BankMsg::Send {
            to_address: taker.to_string(),
            amount: vec![coin_out],
        };

        Ok(Response::new()
            .add_message(refund_msg)
            .add_attribute("action", "reply")
            .add_attribute("reason", "order_execution_failed"))
    }
}
