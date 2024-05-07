use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal, StdResult, Storage};
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

#[cw_serde]
pub enum OrderStatus {
    Open,
    Matched,
    Failed,
    Cancelled,
    Executed,
}

/// Retrieve the number of the next order to be created and increment the counter by one.
pub fn next_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id = COUNTER.may_load(store)?.unwrap_or_default();
    COUNTER.save(store, &(id + 1))?;
    Ok(id)
}

#[cw_serde]
pub struct OrderPointer {
    pub order_id: u64,
    pub maker: Addr,
}

/// Data structure used to store the number of created deals.
pub const COUNTER: Item<u64> = Item::new("counter");
pub const ORDER_POINTER: Item<OrderPointer> = Item::new("order_pointer");
pub const CONFIG: Item<Config> = Item::new("config");
pub const SWAP_ORDERS: Map<(&Addr, u64), SwapOrder> = Map::new("swap_orders");
