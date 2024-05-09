use std::fmt;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, StdResult, Storage};
use cw_storage_plus::{Item, Map};

/// This struct contains configuration parameters for the atomic swap market.
#[cw_serde]
pub struct Config {
    /// Address of the contract owner. This is the only address
    /// that can modify the `Config`.
    pub owner: Addr,
}

/// Contains all information of an order.
#[cw_serde]
pub struct SwapOrder {
    /// Coin that the user wants to swap.
    pub coin_in: Coin,
    /// Coin that the user wants to receive.
    pub coin_out: Coin,
    /// Only address that can accept the deal.
    /// If None, it is an open order. When matched,
    /// it is equal to the taker address.
    pub taker: Option<Addr>,
    /// Timestamp after which the deal expires in seconds.
    pub timeout: u64,
    /// Status of the swap order.
    pub status: OrderStatus,
}

/// Status of a registered order.
#[cw_serde]
pub enum OrderStatus {
    /// Order created and open to be matched.
    Open,
    /// Order Accepted.
    Accepted,
    /// Order confirmed and concluded.
    Confirmed,
    /// Order deleted by the maker.
    Deleted,
    /// Order failed to be executed.
    Failed,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderStatus::Open => write!(f, "Open"),
            OrderStatus::Accepted => write!(f, "Accepted"),
            OrderStatus::Confirmed => write!(f, "Confirmed"),
            OrderStatus::Deleted => write!(f, "Deleted"),
            OrderStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Retrieve the number of the next order to be created and increment the counter by one.
pub fn next_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id = COUNTER.may_load(store)?.unwrap_or_default();
    COUNTER.save(store, &(id + 1))?;
    Ok(id)
}

/// Temporary structure used to store the order that has been
/// confirmed and is waiting to be accepted through `x/authz`
/// message.
#[cw_serde]
pub struct OrderPointer {
    /// Identifier of the order to be accepted.
    pub order_id: u64,
    /// Address of the maker of the order.
    pub maker: Addr,
    /// Address of the taker of the order.
    pub taker: Addr,
}

/// Data structure used to store the number of created deals.
pub const COUNTER: Item<u64> = Item::new("counter");
/// Data structure to store the temporary data of the order being confirmed.
pub const ORDER_POINTER: Item<OrderPointer> = Item::new("order_pointer");
/// Data structure that holds the contract configuration.
pub const CONFIG: Item<Config> = Item::new("config");
/// Data strusture used to store all swap orders.
pub const SWAP_ORDERS: Map<(&Addr, u64), SwapOrder> = Map::new("swap_orders");
