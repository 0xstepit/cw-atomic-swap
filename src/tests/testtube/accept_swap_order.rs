#![cfg(not(tarpaulin_include))]

use cosmwasm_std::Coin;
use osmosis_std::shim::{Any, Timestamp};
use osmosis_std::types::cosmos::authz::v1beta1::{
    Grant, GrantAuthorization, MsgGrant, QueryGranteeGrantsRequest,
};
use osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
use osmosis_std::types::cosmwasm::wasm::v1::{
    AllowAllMessagesFilter, ContractExecutionAuthorization, ContractGrant, MaxFundsLimit,
};

use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisCoin;

use osmosis_test_tube::{Bank, Module, OsmosisTestApp};
use test_tube::cosmrs::proto::prost::Message;
use test_tube::Account;

use crate::msg::{AllSwapOrdersResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::tests::testtube::authz::Authz;
use crate::tests::testtube::test_env::{TestEnvBuilder, WEEK};

#[test]
fn test_accept_swap_order_fails() {
    let app = OsmosisTestApp::new();
    let t = TestEnvBuilder::new()
        .with_account("owner", vec![Coin::new(2_000, "ubtc")])
        .with_account("maker", vec![Coin::new(2_000, "uatom")])
        .with_account(
            "taker",
            vec![Coin::new(1_000, "uatom"), Coin::new(2_000, "usdc")],
        )
        .with_instantiate_msg(InstantiateMsg { owner: None })
        .build(&app);

    let market_address = t.contract.contract_addr.clone();
    let maker = t.accounts.get("maker").unwrap();
    let taker = t.accounts.get("taker").unwrap();

    let bank = Bank::new(&app);

    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: maker.address().to_string(),
            denom: "usdc".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 0u128.to_string(),
            denom: "usdc".to_string(),
        }
    );
    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: taker.address().to_string(),
            denom: "uatom".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 1_000u128.to_string(),
            denom: "uatom".to_string(),
        }
    );

    // ---------------------------------------------------------------------------------------------
    // Update contract state using the grant
    // ---------------------------------------------------------------------------------------------

    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uatom"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: None,
        timeout: 10,
    };

    t.contract.execute(&create_order_msg, &[], maker).unwrap();

    let orders: AllSwapOrdersResponse = t.contract.query(&QueryMsg::AllSwapOrders {}).unwrap();
    assert_eq!(orders.orders.len(), 1);

    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: maker.address().to_string(),
    };
    t.contract
        .execute(&accept_order_msg, &[Coin::new(1_000, "usdc")], taker)
        .unwrap();

    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: maker.address().to_string(),
            denom: "uatom".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 2_000u128.to_string(),
            denom: "uatom".to_string(),
        },
        "expect maker to have original funds"
    );
    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: taker.address().to_string(),
            denom: "usdc".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 2_000u128.to_string(),
            denom: "usdc".to_string(),
        },
        "expect taker to have original funds"
    );
}

#[test]
fn test_accept_swap_order() {
    let app = OsmosisTestApp::new();
    let t = TestEnvBuilder::new()
        .with_account("owner", vec![Coin::new(2_000, "ubtc")])
        .with_account("maker", vec![Coin::new(2_000, "uatom")])
        .with_account("taker", vec![Coin::new(2_000, "usdc")])
        .with_instantiate_msg(InstantiateMsg { owner: None })
        .build(&app);

    let market_address = t.contract.contract_addr.clone();
    let maker = t.accounts.get("maker").unwrap();
    let taker = t.accounts.get("taker").unwrap();

    let authz = Authz::new(&app);
    let bank = Bank::new(&app);

    // ---------------------------------------------------------------------------------------------
    // Create first grant for contract execution
    // ---------------------------------------------------------------------------------------------

    let timestamp = app.get_block_timestamp().seconds() as i64;
    let ts: i64 = timestamp + WEEK;
    let expiration = Timestamp {
        seconds: ts,
        nanos: 0_i32,
    };

    let mut limit_buf = vec![];
    let _ = MaxFundsLimit::encode(
        &MaxFundsLimit {
            amounts: vec![Coin::new(2_000, "uatom").into()],
        },
        &mut limit_buf,
    );

    let mut filter_buf = vec![];
    let _ = AllowAllMessagesFilter::encode(&AllowAllMessagesFilter {}, &mut filter_buf);

    let mut buf = vec![];
    ContractExecutionAuthorization::encode(
        &ContractExecutionAuthorization {
            grants: vec![ContractGrant {
                contract: market_address.clone(),
                limit: Some(Any {
                    type_url: MaxFundsLimit::TYPE_URL.to_string(),
                    value: limit_buf,
                }),
                filter: Some(Any {
                    type_url: AllowAllMessagesFilter::TYPE_URL.to_string(),
                    value: filter_buf,
                }),
            }],
        },
        &mut buf,
    )
    .unwrap();

    // Granter allows the maker to execute contract on its behalf.
    authz
        .grant(
            MsgGrant {
                granter: maker.address(),
                grantee: market_address.clone(),
                grant: Some(Grant {
                    authorization: Some(Any {
                        type_url: ContractExecutionAuthorization::TYPE_URL.to_string(),
                        // type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
                        value: buf.clone(),
                    }),
                    expiration: Some(expiration.clone()),
                }),
            },
            maker,
        )
        .unwrap();

    let response = authz
        .query_grantee_grants(&QueryGranteeGrantsRequest {
            grantee: market_address.clone(),
            pagination: None,
        })
        .unwrap();

    assert_eq!(response.grants.len(), 1);
    assert_eq!(
        response.grants,
        vec![GrantAuthorization {
            granter: maker.address(),
            grantee: market_address.clone(),
            authorization: Some(Any {
                type_url: ContractExecutionAuthorization::TYPE_URL.to_string(),
                // type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
                value: buf.clone(),
            }),
            expiration: Some(expiration.clone()),
        },]
    );

    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: maker.address().to_string(),
            denom: "usdc".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 0u128.to_string(),
            denom: "usdc".to_string(),
        }
    );
    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: taker.address().to_string(),
            denom: "uatom".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 0u128.to_string(),
            denom: "uatom".to_string(),
        }
    );

    // ---------------------------------------------------------------------------------------------
    // Update contract state using the grant
    // ---------------------------------------------------------------------------------------------

    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uatom"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: None,
        timeout: 10,
    };

    t.contract.execute(&create_order_msg, &[], maker).unwrap();

    let orders: AllSwapOrdersResponse = t.contract.query(&QueryMsg::AllSwapOrders {}).unwrap();
    assert_eq!(orders.orders.len(), 1);

    let accept_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: maker.address().to_string(),
    };
    t.contract
        .execute(&accept_order_msg, &[Coin::new(1_000, "usdc")], taker)
        .unwrap();

    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: maker.address().to_string(),
            denom: "usdc".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 1_000u128.to_string(),
            denom: "usdc".to_string(),
        }
    );
    let response = bank
        .query_balance(&QueryBalanceRequest {
            address: taker.address().to_string(),
            denom: "uatom".to_string(),
        })
        .unwrap();
    assert_eq!(
        response.balance.unwrap(),
        OsmosisCoin {
            amount: 1_000u128.to_string(),
            denom: "uatom".to_string(),
        }
    );
}
