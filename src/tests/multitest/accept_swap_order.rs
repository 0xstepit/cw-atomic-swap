use cosmwasm_std::{Addr, Coin, Decimal, Empty, Uint128};
use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};

use crate::msg::{AllSwapOrdersResponse, InstantiateMsg, QueryMsg};
use crate::state::{OrderStatus, SwapOrder};
use crate::{error::ContractError, msg::ExecuteMsg};

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
fn accept_order_without_taker_works() {
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

    // Mint tokens to creator and counterparty
    let mut coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: not_a_scammer.to_string(),
        amount: vec![coin.clone()],
    }))
    .unwrap();

    coin.denom = "usdc".to_string();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: stepit.to_string(),
        amount: vec![coin],
    }))
    .unwrap();

    // Create first order.
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

    // Accept the deal
    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: not_a_scammer.to_string(),
    };
    app.execute_contract(
        stepit.clone(),
        market_addr.clone(),
        &accept_order_msg,
        &[Coin::new(1_000, "usdc")],
    )
    .unwrap();

    let current_block_time = app.block_info().time.seconds();
    let resp_all: AllSwapOrdersResponse = app
        .wrap()
        .query_wasm_smart(market_addr.clone(), &QueryMsg::AllSwapOrders {})
        .unwrap();
    assert_eq!(resp_all.orders.len(), 1, "expected one orders");
    assert_eq!(
        resp_all.orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uosmo"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(stepit),
            status: OrderStatus::Matched,
            timeout: 10 + current_block_time,
        },
        "expected a different order status"
    );
}

#[test]
fn accept_deal_with_counterparty_works() {
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

    // Mint tokens to creator and counterparty
    let mut coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: not_a_scammer.to_string(),
        amount: vec![coin.clone()],
    }))
    .unwrap();

    coin.denom = "usdc".to_string();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: stepit.to_string(),
        amount: vec![coin],
    }))
    .unwrap();

    // Create first order.
    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: Some(stepit.to_string()),
        timeout: 10,
    };
    app.execute_contract(
        not_a_scammer.clone(),
        market_addr.clone(),
        &create_order_msg,
        &[],
    )
    .unwrap();

    // Accept the deal
    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: not_a_scammer.to_string(),
    };
    app.execute_contract(
        stepit.clone(),
        market_addr.clone(),
        &accept_order_msg,
        &[Coin::new(1_000, "usdc")],
    )
    .unwrap();

    let current_block_time = app.block_info().time.seconds();
    let resp_all: AllSwapOrdersResponse = app
        .wrap()
        .query_wasm_smart(market_addr.clone(), &QueryMsg::AllSwapOrders {})
        .unwrap();
    assert_eq!(resp_all.orders.len(), 1, "expected one orders");
    assert_eq!(
        resp_all.orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uosmo"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(stepit),
            status: OrderStatus::Matched,
            timeout: 10 + current_block_time,
        },
        "expected a different order status"
    );
}
//
#[test]
fn accept_order_error_handling() {
    let mut app: App = App::default();

    let owner = Addr::unchecked(OWNER);
    let stepit = Addr::unchecked("0xstepit".to_string());
    let spiderman = Addr::unchecked("0xspider".to_string());
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

    // Mint tokens to maker and taker
    let mut coin = Coin {
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
        amount: vec![coin.clone()],
    }))
    .unwrap();

    coin.denom = "usdc".to_string();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: not_a_scammer.to_string(),
        amount: vec![coin.clone()],
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: stepit.to_string(),
        amount: vec![coin.clone()],
    }))
    .unwrap();
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: spiderman.to_string(),
        amount: vec![coin],
    }))
    .unwrap();

    // Create first order.
    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(500, "usdc"),
        taker: Some(stepit.to_string()),
        timeout: 10,
    };
    app.execute_contract(
        not_a_scammer.clone(),
        market_addr.clone(),
        &create_order_msg,
        &[],
    )
    .unwrap();

    // Let the deal expire
    app.update_block(|block| {
        block.height += 1;
        block.time = block.time.plus_seconds(11);
    });

    // Accept the order
    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: not_a_scammer.to_string(),
    };
    let err = app
        .execute_contract(
            stepit.clone(),
            market_addr.clone(),
            &accept_order_msg,
            &[Coin::new(1_000, "usdc")],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::SwapOrderNotAvailable {},
        "expected error because deal expired"
    );

    // // Time machine to go back when order is not expired
    app.update_block(|block| {
        block.height -= 1;
        block.time = block.time.minus_seconds(11);
    });

    let err = app
        .execute_contract(
            stepit.clone(),
            market_addr.clone(),
            &accept_order_msg,
            &[Coin::new(499, "usdc")],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::WrongCoin {
            denom: "usdc".to_string(),
            amount: Uint128::new(500)
        },
        "expected error because sent tokens are less than the requested"
    );

    let err = app
        .execute_contract(
            stepit.clone(),
            market_addr.clone(),
            &accept_order_msg,
            &[Coin::new(500, "uosmo")],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::WrongCoin {
            denom: "usdc".to_string(),
            amount: Uint128::new(500)
        },
        "expected error because sent token is different than the requested"
    );

    let err = app
        .execute_contract(
            spiderman.clone(),
            market_addr.clone(),
            &accept_order_msg,
            &[Coin::new(500, "usdc")],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::Unauthorized {},
        "expected error because sender is not the requested taker"
    );

    let err = app
        .execute_contract(
            not_a_scammer.clone(),
            market_addr.clone(),
            &accept_order_msg,
            &[Coin::new(500, "usdc")],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::SenderIsMaker {},
        "expected error because maker cannot accept their order"
    );

    // Now the order is accepted correctly
    app.execute_contract(
        stepit.clone(),
        market_addr.clone(),
        &accept_order_msg,
        &[Coin::new(500, "usdc")],
    )
    .unwrap();

    let err = app
        .execute_contract(
            stepit.clone(),
            market_addr.clone(),
            &accept_order_msg,
            &[Coin::new(500, "usdc")],
        )
        .unwrap_err();

    assert_eq!(
        err.downcast_ref::<ContractError>().unwrap(),
        &ContractError::SwapOrderNotAvailable {},
        "expected error because order matched"
    );
}
