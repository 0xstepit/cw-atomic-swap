use cosmwasm_std::{Addr, Coin, Decimal, Empty, Uint128};
use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};

use crate::{
    error::ContractError,
    msg::{AllSwapOrdersResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SwapOrdersByMakerResponse},
};

const OWNER: &str = "0xstepit000";

// Creates a market contract.
pub fn atomic_swap_market_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

#[test]
fn create_order_works() {
    let mut app: App = App::default();

    let owner = Addr::unchecked(OWNER);
    let stepit = Addr::unchecked("0xstepit".to_string());
    let not_a_scammer = Addr::unchecked("0xtrustme".to_string());

    // Store and instantiate the market contract.
    let market_id = app.store_code(atomic_swap_market_contract());
    let init_market_msg = InstantiateMsg {
        owner: Some(OWNER.to_string()),
    };
    let market_addr = app
        .instantiate_contract(
            market_id,
            owner.clone(),
            &init_market_msg,
            &[],
            "atomic-swap-market",
            None,
        )
        .unwrap();

    // Mint tokens to two accounts.
    let coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: not_a_scammer.to_string(),
        amount: vec![coin.clone()],
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: stepit.to_string(),
        amount: vec![coin],
    }))
    .unwrap();

    // Create first order
    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: None,
        timeout: 10,
    };
    app.execute_contract(
        not_a_scammer.clone(),
        market_addr.clone(),
        &create_order_msg,
        &[],
    )
    .unwrap();

    // Create second order with another account
    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: None,
        timeout: 100,
    };
    app.execute_contract(stepit.clone(), market_addr.clone(), &create_order_msg, &[])
        .unwrap();

    let resp: SwapOrdersByMakerResponse = app
        .wrap()
        .query_wasm_smart(
            market_addr.clone(),
            &QueryMsg::SwapOrdersByMaker {
                maker: not_a_scammer.to_string(),
            },
        )
        .unwrap();
    let resp_all: AllSwapOrdersResponse = app
        .wrap()
        .query_wasm_smart(market_addr.clone(), &QueryMsg::AllSwapOrders {})
        .unwrap();

    assert_eq!(resp.orders.len(), 1, "expected one order from the maker");
    assert_eq!(resp_all.orders.len(), 2, "expected two orders");

    // Let the order expire
    app.update_block(|block| {
        block.height += 1;
        block.time = block.time.plus_seconds(11);
    });

    let resp: SwapOrdersByMakerResponse = app
        .wrap()
        .query_wasm_smart(
            market_addr.clone(),
            &QueryMsg::SwapOrdersByMaker {
                maker: not_a_scammer.to_string(),
            },
        )
        .unwrap();
    let resp_all: AllSwapOrdersResponse = app
        .wrap()
        .query_wasm_smart(market_addr.clone(), &QueryMsg::AllSwapOrders {})
        .unwrap();

    assert_eq!(
        resp.orders.len(),
        0,
        "expected zero order from first maker because expired"
    );
    assert_eq!(resp_all.orders.len(), 1, "expected one order still active");
}

#[test]
fn create_order_handle_errors() {
    let mut app: App = App::default();

    let owner = Addr::unchecked(OWNER);
    let not_a_scammer = Addr::unchecked("0xtrustme".to_string());

    // Store and instantiate the market contract.
    let market_id = app.store_code(atomic_swap_market_contract());
    let init_market_msg = InstantiateMsg {
        owner: Some(OWNER.to_string()),
    };
    let market_addr = app
        .instantiate_contract(
            market_id,
            owner.clone(),
            &init_market_msg,
            &[],
            "atomic-swap-market",
            None,
        )
        .unwrap();

    // Mint tokens to two accounts.
    let mut coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: not_a_scammer.to_string(),
        amount: vec![coin.clone()],
    }))
    .unwrap();
    coin.denom = "osmo".to_string();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: not_a_scammer.to_string(),
        amount: vec![coin],
    }))
    .unwrap();

    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(1_000, "uosmo"),
        taker: None,
        timeout: 10,
    };
    let err = app
        .execute_contract(
            not_a_scammer.clone(),
            market_addr.clone(),
            &create_order_msg,
            &[],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::CoinError {
            first_coin: "uosmo".to_string(),
            second_coin: "uosmo".to_string()
        },
        "expected error because sent two coins with same denom"
    );
}
