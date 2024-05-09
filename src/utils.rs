use cosmwasm_std::{Coin, StdError, StdResult};
use osmosis_std::shim::Any;
use osmosis_std::types::cosmos::authz::v1beta1::MsgExec;
use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisCoin;
use osmosis_std::types::cosmwasm::wasm::v1::MsgExecuteContract;
use prost::Message;

use crate::error::{ContractError, EncodeError};
use crate::msg::ExecuteMsg;
use crate::state::{OrderStatus, SwapOrder};

/// Check that the order has a specific status and it is
/// no expired.
pub fn validate_status_and_expiration(
    order: &SwapOrder,
    valid_status: OrderStatus,
    block_time: u64,
) -> Result<(), ContractError> {
    if order.status != valid_status || order.timeout < block_time {
        return Err(ContractError::SwapOrderNotAvailable {
            status: order.status.to_string(),
            expiration: order.timeout,
        });
    };
    Ok(())
}

/// Creates an `x/authz` `MsgExec` encoded message to trigger
/// the confirmation of an order.
pub fn create_authz_encoded_message(
    contract: String,
    order_id: u64,
    maker: String,
    coin: Coin,
) -> Result<MsgExec, ContractError> {
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

    Ok(MsgExec {
        grantee: contract,
        msgs: vec![Any {
            // type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
            type_url: MsgExecuteContract::TYPE_URL.to_string(),
            value: exec_contract_buf.clone(),
        }],
    })
}

/// Check that the two coins are the same or raise an error.
pub fn check_correct_coins(sent_coin: &Coin, expected_coin: &Coin) -> Result<(), ContractError> {
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

/// Check that the two coins are different or raise an error.
pub fn validate_different_denoms(
    denom_in: &String,
    denom_out: &String,
) -> Result<(), ContractError> {
    if denom_in == denom_out {
        return Err(ContractError::SameDenomError {
            denom: denom_in.to_string(),
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

/// Follows cosmos SDK validation logic where denom can be 3 - 128 characters long
/// and starts with a letter, followed but either a letter, number, or separator ( ‘/' , ‘:' , ‘.’ , ‘_’ , or '-')
/// Taken from https://github.com/mars-protocol/red-bank/blob/5bb0fe145588352b281803f7b870103bc6832621/packages/utils/src/helpers.rs#L68
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
