use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal};

use crate::state::{Config, SwapOrder};

/// This struct contains required variables to instantiate a new market.
#[cw_serde]
pub struct InstantiateMsg {
    /// Owner of the smart contract.
    pub owner: Option<String>,
    /// Fee deducted from each exchange in bps.
    pub fee: Decimal,
}

/// This enum describes available contract's execution messages.
#[cw_serde]
pub enum ExecuteMsg {
    /// Allows to update the contract's configuration. Only owner can update.
    UpdateConfig {
        /// New contract owner.
        new_owner: Option<String>,
        /// New swap fee.
        new_fee: Option<Decimal>,
    },
    /// Allows a user to create a swap order. The execution of the order
    /// requires the user to have grant a `ContractExecutionAuthorization`
    /// to this smart contract via the `x/authz` Cosmos SDK module.
    CreateSwapOrder {
        /// Coin to send.
        coin_in: Coin,
        /// Coin to received.
        coin_out: Coin,
        /// If specified, is the only counterparty accepted in the swap.
        taker: Option<Addr>,
        /// Timestamp after which the deal expires in seconds.
        timeout: u64,
    },
    /// Allows a user to match an existing swap order. The function requries
    /// to sent along with the transaction required funds.
    AcceptSwapOrder {
        /// Identifier of the swap order the user wants to match.
        order_id: u64,
    },
    /// This message is sent by the `x/authz` module to complete an swap order
    /// after another user tried to match it with the `AcceptSwapOrder`
    /// `ExecuteMsg`.
    ConfirmSwapOrder {},
}

/// This enum describes available contract's query messages.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Retrieve the market configuration.
    #[returns(Config)]
    Config {},
    #[returns(AllSwapOrdersResponse)]
    /// Retrieve all swap orders.
    AllSwapOrders {},
    /// Retrieve swap orders by creator.
    SwapOrdersByCreator {},
}

#[cw_serde]
pub struct AllSwapOrdersResponse {
    pub deals: Vec<((Addr, u64), SwapOrder)>,
}

#[cw_serde]
pub struct SwapOrdersByCreatorReponse {
    pub deals: Vec<((Addr, u64), SwapOrder)>,
}
