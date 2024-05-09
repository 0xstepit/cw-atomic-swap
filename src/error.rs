use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized,

    #[error("first denom is equal to second denom: {denom}")]
    SameDenomError { denom: String },

    #[error("wrong number of coins: accepted {accepted}, received {received}")]
    FundsError { accepted: u64, received: u64 },

    #[error("this method does not accept coins")]
    CoinNotAllowed {},

    #[error("swap order not available: status {status}, expiration block time {expiration}")]
    SwapOrderNotAvailable { status: String, expiration: u64 },

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

    #[error("unable to encode json")]
    JsonEncodeError(),
}

#[derive(Error, Debug)]
pub enum EncodeError {
    #[error("unable to encode json")]
    JsonEncodeError(#[from] serde_json::Error),
}

impl From<EncodeError> for ContractError {
    fn from(err: EncodeError) -> Self {
        match err {
            EncodeError::JsonEncodeError(_json_err) => {
                ContractError::JsonEncodeError() // Convert to StdError
            }
        }
    }
}
