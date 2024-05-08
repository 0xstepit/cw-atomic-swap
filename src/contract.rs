use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdError, StdResult,
};

use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisCoin;

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{Config, CONFIG},
};

const CONFIRM_ORDER_REPLY_ID: u64 = 1;

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
    use cosmwasm_std::{ensure, Addr, BankMsg, Coin, CosmosMsg, StdError, SubMsg};
    use osmosis_std::shim::Any;
    use osmosis_std::types::cosmos::authz::v1beta1::MsgExec;
    use osmosis_std::types::cosmwasm::wasm::v1::MsgExecuteContract;
    use prost::Message;

    use crate::error::EncodeError;
    use crate::state::{next_id, OrderPointer, OrderStatus, SwapOrder, ORDER_POINTER, SWAP_ORDERS};

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
            .add_attribute("action", "create_order")
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

        // Return error if the order is expired or already matched.
        if order.status != OrderStatus::Open || order.timeout < env.block.time.seconds() {
            return Err(ContractError::SwapOrderNotAvailable {
                status: order.status.to_string(),
            });
        }

        // Check if sent coins are the correct ones.
        check_correct_coins(&info.funds[0], &order.coin_out)?;

        // Check if the order is reserved and the sender is not the lucky one.
        if let Some(taker) = order.taker {
            if taker != info.sender {
                return Err(ContractError::Unauthorized {});
            }
        }

        order.taker = Some(info.sender);
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
        );
        let authz_msg: CosmosMsg = CosmosMsg::Stargate {
            type_url: "/cosmos.authz.v1beta1.MsgExec".to_string(),
            value: msg_exec.into(),
        };
        // TODO: reply on error to take action in case in which the maker
        // doesn't have required funds. What to do? Let's see in the future.
        let msg = SubMsg::reply_on_error(authz_msg, CONFIRM_ORDER_REPLY_ID);

        Ok(Response::new()
            .add_attribute("action", "accept_order")
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
            });
        }

        // Check if sent coins are the same of the selected order.
        check_correct_coins(&info.funds[0], &order.coin_in)?;

        order.status = OrderStatus::Confirmed;
        SWAP_ORDERS.save(deps.storage, (&info.sender, order_id), &order)?;

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
            .add_attribute("action", "confirm_order"))
    }

    /// Check if `sent_coin` is equal to `expected_coin`.
    pub fn check_correct_coins(
        sent_coin: &Coin,
        expected_coin: &Coin,
    ) -> Result<(), ContractError> {
        if sent_coin != expected_coin {
            return Err(ContractError::WrongCoin {
                sent_denom: sent_coin.denom.clone(),
                sent_amount: sent_coin.amount.into(),
                expected_denom: expected_coin.denom.clone(),
                expected_amount: expected_coin.amount.into(),
            });
        }
        Ok(())
    }

    /// Creates an `x/authz` `MsgExec` encoded message to trigger
    /// the confirmation of an order.
    pub fn create_authz_encoded_message(
        contract: String,
        order_id: u64,
        maker: String,
        coin: Coin,
    ) -> MsgExec {
        let update_name_msg = ExecuteMsg::ConfirmSwapOrder {
            order_id,
            maker: maker.clone(),
        };

        let mut exec_contract_buf = vec![];
        MsgExecuteContract::encode(
            &MsgExecuteContract {
                sender: maker.to_string(),
                msg: serde_json::to_vec(&update_name_msg)
                    .map_err(EncodeError::JsonEncodeError)
                    .unwrap(),
                funds: [OsmosisCoin {
                    amount: coin.amount.to_string(),
                    denom: coin.denom,
                }]
                .into(),
                contract: contract.clone(),
            },
            &mut exec_contract_buf,
        )
        .unwrap();

        MsgExec {
            grantee: contract,
            msgs: vec![Any {
                type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
                value: exec_contract_buf.clone(),
            }],
        }
    }

    /// Check that the two coins are different or raise an error.
    pub fn validate_different_denoms(
        denom_in: &String,
        denom_out: &String,
    ) -> Result<(), ContractError> {
        if denom_in == denom_out {
            return Err(ContractError::SameCoinError {
                first_coin: denom_in.to_string(),
                second_coin: denom_out.to_string(),
            });
        }
        Ok(())
    }

    /// Check that only one coin has been sent to the contract.
    pub fn validate_coins_number(funds: &[Coin], allowed_number: u64) -> Result<(), ContractError> {
        if funds.len() as u64 != allowed_number {
            return Err(ContractError::FundsError {
                accepted: allowed_number,
                received: funds.len() as u64,
            });
        }
        Ok(())
    }

    /// Taken from https://github.com/mars-protocol/red-bank/blob/5bb0fe145588352b281803f7b870103bc6832621/packages/utils/src/helpers.rs#L68
    /// Follows cosmos SDK validation logic where denom can be 3 - 128 characters long
    /// and starts with a letter, followed but either a letter, number, or separator ( ‘/' , ‘:' , ‘.’ , ‘_’ , or '-')
    /// reference: https://github.com/cosmos/cosmos-sdk/blob/7728516abfab950dc7a9120caad4870f1f962df5/types/coin.go#L865-L867
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
    use cosmwasm_std::{DepsMut, Response};

    use crate::error::ContractError;
    use crate::state::{OrderPointer, OrderStatus, ORDER_POINTER, SWAP_ORDERS};

    /// Handler the error during the execution of `ConfirmSwapOrder` sent via `x/authz`
    /// module.
    /// NOTE: currently all order status are not used but still included to
    /// easily extend functionalities in the future.
    pub fn reply_confirm_order(deps: DepsMut) -> Result<Response, ContractError> {
        let OrderPointer { order_id, maker } = ORDER_POINTER.load(deps.storage)?;
        SWAP_ORDERS.update(
            deps.storage,
            (&maker, order_id),
            |swap_order| match swap_order {
                Some(mut order) => {
                    order.status = OrderStatus::Failed;
                    Ok(order)
                }
                None => Err(ContractError::Unauthorized),
            },
        )?;
        Ok(Response::new())
    }
}

// -------------------------------------------------------------------------------------------------
// Unit tests
// -------------------------------------------------------------------------------------------------
#[cfg(test)]
mod tests {

    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, Coin, SubMsgResponse, SubMsgResult, Uint128,
    };
    use osmosis_std::types::cosmos::authz::v1beta1::MsgExecResponse;

    use crate::state::{OrderPointer, OrderStatus, SwapOrder, ORDER_POINTER, SWAP_ORDERS};

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

    #[test]
    fn test_validate_coins_number() {
        let funds = vec![Coin {
            denom: "foo".to_string(),
            amount: Uint128::new(100),
        }];
        let result = execute::validate_coins_number(&funds, 1);
        assert!(result.is_ok());

        let funds = vec![
            Coin {
                denom: "foo".to_string(),
                amount: Uint128::new(100),
            },
            Coin {
                denom: "bar".to_string(),
                amount: Uint128::new(200),
            },
        ];
        let result = execute::validate_coins_number(&funds, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_replies() {
        let mut deps = mock_dependencies();

        let owner_addr = Addr::unchecked("0xowner".to_string());
        let maker_addr = Addr::unchecked("0xmaker".to_string());
        let taker_addr = Addr::unchecked("0xtaker".to_string());

        let config = Config { owner: owner_addr };
        CONFIG.save(deps.as_mut().storage, &config).unwrap();

        ORDER_POINTER
            .save(
                deps.as_mut().storage,
                &OrderPointer {
                    order_id: 0,
                    maker: maker_addr.clone(),
                },
            )
            .unwrap();
        let swap_order = SwapOrder {
            coin_in: Coin::new(1_000, "uosmo"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(taker_addr),
            timeout: 10,
            status: OrderStatus::Accepted,
        };
        SWAP_ORDERS
            .save(deps.as_mut().storage, (&maker_addr, 0), &swap_order)
            .unwrap();

        let reply_msg = Reply {
            id: CONFIRM_ORDER_REPLY_ID,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: Some(MsgExecResponse { results: vec![] }.into()),
            }),
        };
        reply(deps.as_mut(), mock_env(), reply_msg).unwrap();
        let swap_order = SWAP_ORDERS
            .load(deps.as_mut().storage, (&maker_addr, 0))
            .unwrap();
        assert_eq!(swap_order.status, OrderStatus::Failed);
    }
}
