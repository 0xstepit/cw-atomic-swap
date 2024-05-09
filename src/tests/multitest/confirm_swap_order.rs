use cosmwasm_std::{Addr, Coin, Empty, Uint128};
use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};

use crate::{
    error::ContractError,
    msg::{AllSwapOrdersResponse, ExecuteMsg, InstantiateMsg, QueryMsg, SwapOrdersByMakerResponse},
};

pub fn atomic_swap_market_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

#[test]
fn test_create_order() {
    let mut app: App = App::default();

    let owner = Addr::unchecked("0xowner".to_string());
    let stepit = Addr::unchecked("0xstepit".to_string());
    let maker = Addr::unchecked("maker".to_string());

    // Store and instantiate the market contract.
    let market_id = app.store_code(atomic_swap_market_contract());
    let init_market_msg = InstantiateMsg {
        owner: Some("0xowner".to_string()),
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
        to_address: maker.to_string(),
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
    app.execute_contract(maker.clone(), market_addr.clone(), &create_order_msg, &[])
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
                maker: maker.to_string(),
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
                maker: maker.to_string(),
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
