use cosmwasm_std::{Coin, StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized,

    #[error("fuck you")]
    Fuck,

    #[error("first coin {first_coin} is equal to second coin {second_coin}")]
    CoinError {
        first_coin: String,
        second_coin: String,
    },

    #[error("wrong number of coins: accepted {accepted}, received {received}")]
    FundsError { accepted: u64, received: u64 },

    #[error("this method does not accept coins")]
    CoinNotAllowed {},

    #[error("swap order not available: expired or already matched")]
    SwapOrderNotAvailable {},

    #[error(
        "sent wrong coins {sent_denom}{sent_amount}, expected {expected_amount}{expected_denom}"
    )]
    WrongCoin {
        sent_denom: String,
        sent_amount: u128,
        expected_denom: String,
        expected_amount: u128,
    },

    #[error("maker cannot accept its own order")]
    SenderIsMaker {},
}

#[derive(Error, Debug)]
pub enum EncodeError {
    #[error("unable to encode json")]
    JsonEncodeError(#[from] serde_json::Error),
}
