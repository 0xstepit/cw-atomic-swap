use cosmwasm_std::{
    coin, entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, CONFIG},
};

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

    CONFIG.save(deps.storage, &Config { owner: info.sender })?;

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
        UpdateConfig { new_owner, new_fee } => unimplemented!(),
        CreateSwapOrder {
            coin_in,
            coin_out,
            taker,
            timeout,
        } => execute::create_swap_order(deps, env, info, coin_in, coin_out, taker, timeout),
        AcceptSwapOrder { order_id } => unimplemented!(),
        ConfirmSwapOrder {} => unimplemented!(),
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

pub mod execute {
    use cosmwasm_std::{Coin, StdError};

    use crate::state::{next_id, OrderStatus, SwapOrder, SWAP_ORDERS};

    use super::*;

    /// Crerate a new deal. The deal can be open of specific for one counterparty.
    pub fn create_swap_order(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        coin_in: Coin,
        coin_out: Coin,
        taker: Option<String>,
        timeout: u64,
    ) -> Result<Response, ContractError> {
        let config = CONFIG.load(deps.storage)?;

        if coin_in.denom == coin_out.denom {
            return Err(ContractError::CoinError {
                first_coin: coin_in.denom,
                second_coin: coin_out.denom,
            });
        }

        validate_native_denom(&coin_in.denom)?;
        validate_native_denom(&coin_out.denom)?;
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
            .add_attribute("action", "create_order")
            .add_attribute("order_id", order_id.to_string())
            .add_attribute("maker", info.sender))
    }

    /// Taken from https://github.com/mars-protocol/red-bank/blob/5bb0fe145588352b281803f7b870103bc6832621/packages/utils/src/helpers.rs#L68
    /// Follows cosmos SDK validation logic where denom can be 3 - 128 characters long
    /// and starts with a letter, followed but either a letter, number, or separator ( ‘/' , ‘:' , ‘.’ , ‘_’ , or '-')
    /// reference: https://github.com/cosmos/cosmos-sdk/blob/7728516abfab950dc7a9120caad4870f1f962df5/types/coin.go#L865-L867
    /// NOTE: tests are in their repo so no unit tests made for this function.
    pub fn validate_native_denom(denom: &str) -> StdResult<()> {
        if denom.len() < 3 || denom.len() > 128 {
            return Err(StdError::generic_err(format!(
                "invalid denom length [3,128]: {denom}"
            )));
        }

        let mut chars = denom.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_alphabetic() {
            return Err(StdError::generic_err(format!(
                "first character is not ASCII alphabetic: {denom}"
            )));
        }

        let set = ['/', ':', '.', '_', '-'];
        for c in chars {
            if !(c.is_ascii_alphanumeric() || set.contains(&c)) {
                return Err(StdError::generic_err(format!(
                    "not all characters are ASCII alphanumeric or one of:  /  :  .  _  -: {denom}"
                )));
            }
        }

        Ok(())
    }
}

pub mod query {

    use cosmwasm_std::{Addr, Order};

    use crate::msg::{AllSwapOrdersResponse, SwapOrdersByMakerResponse};
    use crate::state::{SwapOrder, SWAP_ORDERS};

    use super::*;

    pub fn get_config(deps: Deps) -> StdResult<Config> {
        CONFIG.load(deps.storage)
    }

    /// Returns all active deals.
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

    /// Returns the active deals associated with a creator.
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

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {

    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        Addr,
    };

    use common::market::InstantiateMsg;

    use super::*;

    #[test]
    fn instatiate_works() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = mock_info("stepit", &[]);

        instantiate(
            deps.as_mut(),
            env,
            info,
            InstantiateMsg {
                owner: Some("stepit".to_string()),
            },
        )
        .unwrap();

        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        let expected_config = Config {
            owner: Addr::unchecked("stepit"),
        };
        assert_eq!(expected_config, config, "expected different config")
    }
}
