use std::str::FromStr;

use cosmwasm_std::{ensure_eq, Addr, Coin, Decimal};
use osmosis_std::shim::{Any, Timestamp};
use osmosis_std::types::cosmos::authz::v1beta1::{
    Grant, GrantAuthorization, MsgExec, MsgGrant, QueryGranteeGrantsRequest,
};
use osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest;
use osmosis_std::types::cosmwasm::wasm::v1::{
    AllowAllMessagesFilter, ContractExecutionAuthorization, ContractGrant, MaxFundsLimit,
    MsgExecuteContract,
};

use osmosis_std::types::cosmos::base::v1beta1::Coin as OsmosisCoin;

use osmosis_test_tube::{Bank, Module, OsmosisTestApp, Wasm};
use test_tube::cosmrs::proto::prost::Message;
use test_tube::{Account, EncodeError};

use crate::msg::{AllSwapOrdersResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::Config;
use crate::tests::testtube::authz::Authz;
use crate::tests::testtube::test_env::{TestEnvBuilder, WEEK};

#[test]
fn test_submessage() {
    let app = OsmosisTestApp::new();
    let t = TestEnvBuilder::new()
        .with_account("owner", vec![Coin::new(2_000, "ubtc")])
        // .with_account("granter", vec![Coin::new(2_000, "ubtc")])
        .with_account("maker", vec![Coin::new(2_000, "uatom")])
        .with_account("taker", vec![Coin::new(2_000, "usdc")])
        .with_instantiate_msg(InstantiateMsg { owner: None })
        .build(&app);

    let market_address = t.contract.contract_addr.clone();
    println!("{}", market_address);
    let maker = t.accounts.get("maker").unwrap();
    let taker = t.accounts.get("taker").unwrap();
    println!("{}", maker.address());
    println!("{}", taker.address());

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
                    type_url: "/cosmwasm.wasm.v1.MaxFundsLimit".to_string(),
                    value: limit_buf,
                }),
                filter: Some(Any {
                    type_url: "/cosmwasm.wasm.v1.AllowAllMessagesFilter".to_string(),
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
                        type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
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
                type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
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

// #[test]
// fn test_setup_with_authz() {
//     let app = OsmosisTestApp::new();
//     let t = TestEnvBuilder::new()
//         .with_account("owner", vec![])
//         .with_account("granter", vec![Coin::new(2_000, "ubtc")])
//         .with_account("maker", vec![Coin::new(2_000, "uatom")])
//         .with_account("taker", vec![Coin::new(2_000, "usdc")])
//         .with_instantiate_msg(InstantiateMsg {
//             owner: None,
//             fee: Decimal::from_str(FEE).unwrap(),
//         })
//         .build(&app);
//
//     let market_address = t.contract.contract_addr.clone();
//     let owner = t.owner;
//     let granter = t.accounts.get("granter").unwrap();
//     let maker = t.accounts.get("maker").unwrap();
//     let owner = t.accounts.get("owner").unwrap();
//
//     let authz = Authz::new(&app);
//     let bank = Bank::new(&app);
//
//     // ---------------------------------------------------------------------------------------------
//     // Ensure not grants
//     // ---------------------------------------------------------------------------------------------
//
//     let response = authz
//         .query_grantee_grants(&QueryGranteeGrantsRequest {
//             grantee: maker.address().clone(),
//             pagination: None,
//         })
//         .unwrap();
//
//     assert_eq!(response.grants, vec![]);
//     assert_eq!(t.contract.code_id, 1);
//
//     // ---------------------------------------------------------------------------------------------
//     // Create first grant for contract execution
//     // ---------------------------------------------------------------------------------------------
//
//     let timestamp = app.get_block_timestamp().seconds() as i64;
//     let ts: i64 = timestamp + WEEK;
//     let expiration = Timestamp {
//         seconds: ts,
//         nanos: 0_i32,
//     };
//
//     let mut limit_buf = vec![];
//     let _ = MaxFundsLimit::encode(
//         &MaxFundsLimit {
//             amounts: vec![Coin::new(1_000, "ubtc").into()],
//         },
//         &mut limit_buf,
//     );
//
//     let mut filter_buf = vec![];
//     let _ = AllowAllMessagesFilter::encode(&AllowAllMessagesFilter {}, &mut filter_buf);
//
//     let mut buf = vec![];
//     ContractExecutionAuthorization::encode(
//         &ContractExecutionAuthorization {
//             grants: vec![ContractGrant {
//                 contract: market_address.clone(),
//                 limit: Some(Any {
//                     type_url: "/cosmwasm.wasm.v1.MaxFundsLimit".to_string(),
//                     value: limit_buf,
//                 }),
//                 filter: Some(Any {
//                     type_url: "/cosmwasm.wasm.v1.AllowAllMessagesFilter".to_string(),
//                     value: filter_buf,
//                 }),
//             }],
//         },
//         &mut buf,
//     )
//     .unwrap();
//
//     // Contract owner allows the maker to execute contract on its behalf.
//     authz
//         .grant(
//             MsgGrant {
//                 granter: granter.address(),
//                 grantee: maker.address().clone(),
//                 grant: Some(Grant {
//                     authorization: Some(Any {
//                         type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
//                         value: buf.clone(),
//                     }),
//                     expiration: Some(expiration.clone()),
//                 }),
//             },
//             &granter,
//         )
//         .unwrap();
//
//     let response = authz
//         .query_grantee_grants(&QueryGranteeGrantsRequest {
//             grantee: maker.address().clone(),
//             pagination: None,
//         })
//         .unwrap();
//
//     assert_eq!(response.grants.len(), 1);
//     assert_eq!(
//         response.grants,
//         vec![GrantAuthorization {
//             granter: granter.address(),
//             grantee: maker.address().clone(),
//             authorization: Some(Any {
//                 type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
//                 value: buf.clone(),
//             }),
//             expiration: Some(expiration.clone()),
//         },]
//     );
//
//     // ---------------------------------------------------------------------------------------------
//     // Verify initial contract state
//     // ---------------------------------------------------------------------------------------------
//     let query_msg = QueryMsg::Counter {};
//     let resp_counter: Counter = t.contract.query(&query_msg).unwrap();
//     assert_eq!(
//         resp_counter,
//         Counter {
//             counter: 0,
//             sender: "".to_string(),
//         }
//     );
//
//     let response = bank
//         .query_balance(&QueryBalanceRequest {
//             address: market_address.to_string(),
//             denom: "ubtc".to_string(),
//         })
//         .unwrap();
//     assert_eq!(
//         response.balance.unwrap(),
//         BaseCoin {
//             amount: 0u128.to_string(),
//             denom: "ubtc".to_string(),
//         }
//     );
//
//     // ---------------------------------------------------------------------------------------------
//     // Update contract state using the grant
//     // ---------------------------------------------------------------------------------------------
//
//     // Update the counter value and send half of authorized tokens.
//     let exec_msg = ExecuteMsg::UpdateCounter {};
//     let mut exec_buf = vec![];
//     MsgExecuteContract::encode(
//         &MsgExecuteContract {
//             sender: granter.address().to_string(),
//             msg: serde_json::to_vec(&exec_msg)
//                 .map_err(EncodeError::JsonEncodeError)
//                 .unwrap(),
//             funds: [BaseCoin {
//                 amount: 500u128.to_string(),
//                 denom: "ubtc".to_string(),
//             }]
//             .into(),
//             contract: market_address.clone(),
//         },
//         &mut exec_buf,
//     )
//     .unwrap();
//
//     authz
//         .exec(
//             MsgExec {
//                 grantee: maker.address().to_string(),
//                 msgs: vec![Any {
//                     type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
//                     value: exec_buf.clone(),
//                 }],
//             },
//             maker,
//         )
//         .unwrap();
//
//     // ---------------------------------------------------------------------------------------------
//     // Verify contract state has been updated and tokens have been removed
//     // ---------------------------------------------------------------------------------------------
//
//     let resp_counter: Counter = t.contract.query(&query_msg).unwrap();
//     assert_eq!(
//         resp_counter,
//         Counter {
//             counter: 1,
//             sender: granter.address().to_string(),
//         }
//     );
//
//     let response = bank
//         .query_balance(&QueryBalanceRequest {
//             address: granter.address().to_string(),
//             denom: "ubtc".to_string(),
//         })
//         .unwrap();
//     assert_eq!(
//         response.balance.unwrap(),
//         BaseCoin {
//             amount: 1_500.to_string(),
//             denom: "ubtc".to_string(),
//         }
//     );
//
//     let response = bank
//         .query_balance(&QueryBalanceRequest {
//             address: market_address.to_string(),
//             denom: "ubtc".to_string(),
//         })
//         .unwrap();
//     assert_eq!(
//         response.balance.unwrap(),
//         BaseCoin {
//             amount: 500.to_string(),
//             denom: "ubtc".to_string(),
//         }
//     );
//
//     // ---------------------------------------------------------------------------------------------
//     // Verify authz limit respected
//     // ---------------------------------------------------------------------------------------------
//     authz
//         .exec(
//             MsgExec {
//                 grantee: maker.address().to_string(),
//                 msgs: vec![Any {
//                     type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
//                     value: exec_buf.clone(),
//                 }],
//             },
//             maker,
//         )
//         .unwrap();
//
//     let resp_counter: Counter = t.contract.query(&query_msg).unwrap();
//     assert_eq!(
//         resp_counter,
//         Counter {
//             counter: 2,
//             sender: granter.address().to_string(),
//         }
//     );
//
//     // Cannot handle the unwrap err because of thread 'tests::authz_test::test_setup_with_authz' panicked at /Users/stepit/.cargo/registry/src/index.crates.io-6f17d22bba15001f/test-tube-0.6.0/src/runner/result.rs:222:18:
//     // authz
//     //     .exec(
//     //         MsgExec {
//     //             grantee: maker.address().to_string(),
//     //             msgs: vec![Any {
//     //                 type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
//     //                 value: exec_buf.clone(),
//     //             }],
//     //         },
//     //         maker,
//     //     )
//     //     .unwrap_err();
// }
//
// #[test]
// fn test_new_owner() {
//     let app = OsmosisTestApp::new();
//     let t = TestEnvBuilder::new()
//         .with_account("owner", vec![Coin::new(2_000, "ubtc")])
//         .with_account("granter", vec![Coin::new(2_000, "ubtc")])
//         .with_account("maker", vec![Coin::new(2_000, "uatom")])
//         .with_account("taker", vec![Coin::new(2_000, "usdc")])
//         .with_instantiate_msg(InstantiateMsg {
//             owner: None,
//             fee: Decimal::from_str(FEE).unwrap(),
//         })
//         .build(&app);
//
//     let market_address = t.contract.contract_addr.clone();
//     let owner = t.owner;
//     let granter = t.accounts.get("granter").unwrap();
//     let maker = t.accounts.get("maker").unwrap();
//     let owner = t.accounts.get("owner").unwrap();
//
//     let authz = Authz::new(&app);
//     let bank = Bank::new(&app);
//
//     // ---------------------------------------------------------------------------------------------
//     // Create first grant for contract execution
//     // ---------------------------------------------------------------------------------------------
//
//     let timestamp = app.get_block_timestamp().seconds() as i64;
//     let ts: i64 = timestamp + WEEK;
//     let expiration = Timestamp {
//         seconds: ts,
//         nanos: 0_i32,
//     };
//
//     let mut limit_buf = vec![];
//     let _ = MaxFundsLimit::encode(
//         &MaxFundsLimit {
//             amounts: vec![Coin::new(1_000, "ubtc").into()],
//         },
//         &mut limit_buf,
//     );
//
//     let mut filter_buf = vec![];
//     let _ = AllowAllMessagesFilter::encode(&AllowAllMessagesFilter {}, &mut filter_buf);
//
//     let mut buf = vec![];
//     ContractExecutionAuthorization::encode(
//         &ContractExecutionAuthorization {
//             grants: vec![ContractGrant {
//                 contract: market_address.clone(),
//                 limit: Some(Any {
//                     type_url: "/cosmwasm.wasm.v1.MaxFundsLimit".to_string(),
//                     value: limit_buf,
//                 }),
//                 filter: Some(Any {
//                     type_url: "/cosmwasm.wasm.v1.AllowAllMessagesFilter".to_string(),
//                     value: filter_buf,
//                 }),
//             }],
//         },
//         &mut buf,
//     )
//     .unwrap();
//
//     // Granter allows the maker to execute contract on its behalf.
//     authz
//         .grant(
//             MsgGrant {
//                 granter: owner.address(),
//                 grantee: maker.address().clone(),
//                 grant: Some(Grant {
//                     authorization: Some(Any {
//                         type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
//                         value: buf.clone(),
//                     }),
//                     expiration: Some(expiration.clone()),
//                 }),
//             },
//             &owner,
//         )
//         .unwrap();
//
//     let response = authz
//         .query_grantee_grants(&QueryGranteeGrantsRequest {
//             grantee: maker.address().clone(),
//             pagination: None,
//         })
//         .unwrap();
//
//     assert_eq!(response.grants.len(), 1);
//     assert_eq!(
//         response.grants,
//         vec![GrantAuthorization {
//             granter: owner.address(),
//             grantee: maker.address().clone(),
//             authorization: Some(Any {
//                 type_url: "/cosmwasm.wasm.v1.ContractExecutionAuthorization".to_string(),
//                 value: buf.clone(),
//             }),
//             expiration: Some(expiration.clone()),
//         },]
//     );
//
//     // ---------------------------------------------------------------------------------------------
//     // Verify initial contract state
//     // ---------------------------------------------------------------------------------------------
//     let query_msg = QueryMsg::Config {};
//     let resp_config: Config = t.contract.query(&query_msg).unwrap();
//     assert_eq!(
//         resp_config,
//         Config {
//             owner: Addr::unchecked(owner.address()),
//             fee: Decimal::from_str(FEE).unwrap(),
//         }
//     );
//
//     // ---------------------------------------------------------------------------------------------
//     // Update contract state using the grant
//     // ---------------------------------------------------------------------------------------------
//
//     let exec_msg = ExecuteMsg::UpdateName {
//         new_name: "ste".to_string(),
//     };
//     let mut exec_buf = vec![];
//     MsgExecuteContract::encode(
//         &MsgExecuteContract {
//             sender: owner.address().to_string(),
//             msg: serde_json::to_vec(&exec_msg)
//                 .map_err(EncodeError::JsonEncodeError)
//                 .unwrap(),
//             funds: [BaseCoin {
//                 amount: 500u128.to_string(),
//                 denom: "ubtc".to_string(),
//             }]
//             .into(),
//             contract: market_address.clone(),
//         },
//         &mut exec_buf,
//     )
//     .unwrap();
//
//     authz
//         .exec(
//             MsgExec {
//                 grantee: maker.address().to_string(),
//                 msgs: vec![Any {
//                     type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
//                     value: exec_buf.clone(),
//                 }],
//             },
//             maker,
//         )
//         .unwrap();
//
//     let query_msg = QueryMsg::Name {};
//     let resp_config: String = t.contract.query(&query_msg).unwrap();
//     assert_eq!(resp_config, "ste".to_string(),);
// }
