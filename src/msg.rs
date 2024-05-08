use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Coin};

use crate::state::{Config, SwapOrder};

/// This structure contains required variables to instantiate a new market.
#[cw_serde]
pub struct InstantiateMsg {
    /// Owner of the smart contract.
    pub owner: Option<String>,
}

/// This enum describes available contract's execution messages.
#[cw_serde]
pub enum ExecuteMsg {
    /// Allows to update the contract's configuration.
    /// Only owner can update.
    UpdateConfig {
        /// New contract owner.
        new_owner: String,
    },
    /// Allows a user to create a swap order. The execution of the order
    /// requires the user to have granted a `ContractExecutionAuthorization`
    /// to this smart contract via the `x/authz` Cosmos SDK module with the
    /// allowance to spend `coin_in`.
    CreateSwapOrder {
        /// Coin to send.
        coin_in: Coin,
        /// Coin to received.
        coin_out: Coin,
        /// If specified, is the only counterparty accepted in the swap.
        taker: Option<String>,
        /// Timestamp after which the deal expires in seconds.
        timeout: u64,
    },
    /// Allows a user to accept an existing swap order. The function requires
    /// to send along with the transaction required funds.
    AcceptSwapOrder {
        /// Identifier of the swap order the user wants to match.
        order_id: u64,
        /// The maker associated with the order.
        // TODO: add a way to retrieve an order from the id for a better UX.
        maker: String,
    },
    /// This message is sent by the `x/authz` module to complete an swap order
    /// after another user tried to match it with the `AcceptSwapOrder`
    /// `ExecuteMsg`.
    ConfirmSwapOrder {
        /// Identifier of the swap order to confirm.
        order_id: u64,
        /// The maker associated with the order.
        // TODO: add a way to retrieve an order from the id for a better UX.
        maker: String,
    },
}

/// This enum describes available contract's query messages.
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Retrieve the market configuration.
    #[returns(Config)]
    Config {},
    // TODO: in both query below add a flag to specify if
    // timedout orders are wanted. (how to cancel expired ones otherwise?)
    #[returns(AllSwapOrdersResponse)]
    /// Retrieve all active swap orders.
    AllSwapOrders {},
    #[returns(SwapOrdersByMakerResponse)]
    /// Retrieve all active swap orders by maker.
    SwapOrdersByMaker { maker: String },
}

/// Data structure returned from the `AllSwapOrders` query.
#[cw_serde]
pub struct AllSwapOrdersResponse {
    pub orders: Vec<((Addr, u64), SwapOrder)>,
}

/// Data structure returned from the `SwapOrdersByMaker` query.
#[cw_serde]
pub struct SwapOrdersByMakerResponse {
    pub orders: Vec<(u64, SwapOrder)>,
}
