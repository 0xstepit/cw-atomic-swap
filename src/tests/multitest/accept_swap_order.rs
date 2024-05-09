use cosmwasm_std::{Addr, Coin, Empty, Uint128};
use cw_multi_test::{AppBuilder, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};

use crate::msg::{ExecuteMsg, SwapOrdersByMakerResponse};
use crate::msg::{InstantiateMsg, QueryMsg};
use crate::state::{OrderStatus, SwapOrder};
use crate::tests::multitest::stargate_handler::CustomStargate;

const OWNER: &str = "0xstepit000";

// Creates a market contract.
pub fn atomic_swap_market_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply_empty(crate::contract::reply);

    Box::new(contract)
}

#[test]
fn test_accept_swap_order_no_taker() {
    let mut app = AppBuilder::new()
        .with_stargate(CustomStargate::default())
        .build(|_, _, _| {});

    let owner = Addr::unchecked(OWNER);
    let stepit = Addr::unchecked("0xstepit".to_string());
    let maker = Addr::unchecked("maker".to_string());

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
        to_address: maker.to_string(),
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
    app.execute_contract(maker.clone(), market_addr.clone(), &create_order_msg, &[])
        .unwrap();

    // Accept the order
    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: maker.to_string(),
    };
    app.execute_contract(
        stepit.clone(),
        market_addr.clone(),
        &accept_order_msg,
        &[Coin::new(1_000, "usdc")],
    )
    .unwrap();
}

#[test]
fn test_accept_swap_order_with_taker() {
    let mut app = AppBuilder::new()
        .with_stargate(CustomStargate::default())
        .build(|_, _, _| {});

    let owner = Addr::unchecked(OWNER);
    let stepit = Addr::unchecked("0xstepit".to_string());
    let maker = Addr::unchecked("maker".to_string());

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
        to_address: maker.to_string(),
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
    app.execute_contract(maker.clone(), market_addr.clone(), &create_order_msg, &[])
        .unwrap();

    // Accept the order
    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: maker.to_string(),
    };
    app.execute_contract(
        stepit.clone(),
        market_addr.clone(),
        &accept_order_msg,
        &[Coin::new(1_000, "usdc")],
    )
    .unwrap();

    let resp: SwapOrdersByMakerResponse = app
        .wrap()
        .query_wasm_smart(
            market_addr.clone(),
            &QueryMsg::SwapOrdersByMaker {
                maker: maker.to_string(),
            },
        )
        .unwrap();

    let current_block_time = app.block_info().time.seconds();
    assert_eq!(
        resp.orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uosmo"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(stepit),
            timeout: 10 + current_block_time,
            status: OrderStatus::Accepted,
        },
        "expected the order to be accepted"
    )
}

#[test]
fn test_accept_swap_order_trigger_reply() {
    let mut app = AppBuilder::new()
        .with_stargate(CustomStargate::default())
        .build(|_, _, _| {});

    let owner = Addr::unchecked(OWNER);
    let stepit = Addr::unchecked("0xstepit".to_string());
    let maker = Addr::unchecked("maker".to_string());

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
        to_address: maker.to_string(),
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
    app.execute_contract(maker.clone(), market_addr.clone(), &create_order_msg, &[])
        .unwrap();

    // Accept the order
    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: maker.to_string(),
    };

    let mut current_block = app.block_info();
    let current_block_time = current_block.time.seconds();
    current_block.height = 1;
    app.set_block(current_block);
    app.execute_contract(
        stepit.clone(),
        market_addr.clone(),
        &accept_order_msg,
        &[Coin::new(1_000, "usdc")],
    )
    .unwrap();

    let resp: SwapOrdersByMakerResponse = app
        .wrap()
        .query_wasm_smart(
            market_addr.clone(),
            &QueryMsg::SwapOrdersByMaker {
                maker: maker.to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        resp.orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uosmo"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(stepit),
            timeout: 10 + current_block_time,
            status: OrderStatus::Failed,
        },
        "expected the order to be failed because error in submessage"
    );
}
