use cosmwasm_std::{
    coin, entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult,
};

use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisCoin;

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
        AcceptSwapOrder { order_id, maker } => {
            execute::accept_swap_order(deps, info, env, order_id, maker)
        }
        ConfirmSwapOrder { order_id, maker } => unimplemented!(),
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
    use cosmwasm_std::{Addr, BankMsg, Coin, CosmosMsg, StdError};
    use osmosis_std::shim::Any;
    use osmosis_std::types::cosmos::authz::v1beta1::MsgExec;
    use osmosis_std::types::cosmwasm::wasm::v1::MsgExecuteContract;
    use prost::Message;

    use crate::error::EncodeError;
    use crate::state::{next_id, OrderStatus, SwapOrder, SWAP_ORDERS};

    use super::*;

    /// Create a new atomic swap order.
    ///
    /// # Errors
    ///
    /// - coins sent to the contract along with the message.
    /// - coins to swap are not native.
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
        validate_coins_number(&info.funds, 1)?;

        // We don't care about validation because the address is not stored.
        let maker = Addr::unchecked(maker);
        if info.sender == maker {
            return Err(ContractError::SenderIsMaker {});
        }
        let mut order = SWAP_ORDERS.load(deps.storage, (&maker, order_id))?;

        // Return error if the order is expired or already matched.
        if order.status != OrderStatus::Open || order.timeout < env.block.time.seconds() {
            return Err(ContractError::SwapOrderNotAvailable {});
        }

        // Check if sent coins are the same of the selected order.
        if order.coin_out != info.funds[0] {
            return Err(ContractError::WrongCoin {
                denom: order.coin_out.denom.clone(),
                amount: order.coin_out.amount,
            });
        }

        // Check if the order is reserved and sender is not the lucky one.
        if let Some(taker) = order.taker {
            if taker != info.sender {
                return Err(ContractError::Unauthorized {});
            }
        }

        order.taker = Some(info.sender);
        order.status = OrderStatus::Matched;

        SWAP_ORDERS.save(deps.storage, (&maker, order_id), &order)?;

        let msg_exec = create_authz_encoded_message(
            deps.as_ref(),
            env.contract.address.to_string(),
            order_id,
            maker.to_string(),
            order.coin_in,
        );
        let authz_msg: CosmosMsg = CosmosMsg::Stargate {
            type_url: "/cosmos.authz.v1beta1.MsgExec".to_string(),
            value: msg_exec.into(),
        };

        Ok(Response::new()
            .add_attribute("action", "match_order")
            .add_attribute("order_taker", order.taker.unwrap())
            .add_message(authz_msg))
    }

    pub fn create_authz_encoded_message(
        deps: Deps,
        contract: String,
        order_id: u64,
        maker: String,
        coin: Coin,
    ) -> MsgExec {
        deps.api.debug("MsgExec built");
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

        deps.api.debug("Completed MsgExecuteContract encoding");

        MsgExec {
            grantee: contract,
            msgs: vec![Any {
                type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
                value: exec_contract_buf.clone(),
            }],
        }
    }

    pub fn confirm_swap_order(
        deps: DepsMut,
        info: MessageInfo,
        env: Env,
        order_id: u64,
        maker: String,
    ) -> Result<Response, ContractError> {
        validate_coins_number(&info.funds, 1)?;

        if maker != info.sender {
            return Err(ContractError::Unauthorized {});
        }

        let mut order = SWAP_ORDERS.load(deps.storage, (&info.sender, order_id))?;

        // Return error if the order is expired or already matched.
        // NOTE: order should not be timeouted.
        if order.status != OrderStatus::Matched || order.timeout < env.block.time.seconds() {
            return Err(ContractError::SwapOrderNotAvailable {});
        }

        // Check if sent coins are the same of the selected order.
        if order.coin_in != info.funds[0] {
            return Err(ContractError::WrongCoin {
                denom: order.coin_in.denom.clone(),
                amount: order.coin_in.amount,
            });
        }

        order.status = OrderStatus::Executed;
        SWAP_ORDERS.save(deps.storage, (&info.sender, order_id), &order)?;

        let taker = order.taker.unwrap();
        let mut msgs = vec![
            BankMsg::Send {
                to_address: maker.into(),
                amount: vec![order.coin_out],
            },
            BankMsg::Send {
                to_address: taker.clone().into_string(),
                amount: vec![order.coin_in],
            },
        ];

        Ok(Response::new()
            .add_attribute("action", "match_order")
            .add_attribute("order_taker", taker.into_string()))
    }

    /// Check that only one coin has been sent to the contract.
    pub fn validate_different_denoms(
        denom_in: &String,
        denom_out: &String,
    ) -> Result<(), ContractError> {
        if denom_in == denom_out {
            return Err(ContractError::CoinError {
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
        Addr, Coin, Uint128,
    };

    use self::tests::execute::validate_coins_number;

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
        let result = validate_coins_number(&funds, 1);
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
        let result = validate_coins_number(&funds, 1);
        assert!(result.is_err());
    }
}
