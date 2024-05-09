use cosmwasm_std::{
    from_json,
    testing::{mock_dependencies, mock_env, mock_info},
    Addr, Coin, Reply, SubMsgResponse, SubMsgResult, Uint128,
};
use osmosis_std::types::cosmos::authz::v1beta1::MsgExecResponse;

use crate::{
    contract::{execute, instantiate, query, reply, CONFIRM_ORDER_REPLY_ID},
    error::ContractError,
    msg::{InstantiateMsg, QueryMsg, SwapOrdersByMakerResponse},
    state::Config,
};
use crate::{
    msg::ExecuteMsg,
    state::{OrderPointer, OrderStatus, SwapOrder, CONFIG, ORDER_POINTER, SWAP_ORDERS},
};

use crate::utils;

#[test]
fn test_instatiate() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("ste", &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            owner: Some("pit".to_string()),
        },
    )
    .unwrap();

    let config = CONFIG.load(deps.as_ref().storage).unwrap();
    let expected_config = Config {
        owner: Addr::unchecked("pit"),
    };
    assert_eq!(
        expected_config, config,
        "expected specified owner in config"
    );

    let mut deps = mock_dependencies();
    instantiate(deps.as_mut(), env, info, InstantiateMsg { owner: None }).unwrap();

    let config = CONFIG.load(deps.as_ref().storage).unwrap();
    let expected_config = Config {
        owner: Addr::unchecked("ste"),
    };
    assert_eq!(expected_config, config, "expected info sender as owner");
}

#[test]
fn test_instatiate_handling_errors() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("stepit", &[]);

    let err = instantiate(
        deps.as_mut(),
        env,
        info,
        InstantiateMsg {
            owner: Some("".to_string()),
        },
    );

    assert!(
        err.is_err(),
        "expected failing instantiation when non valid owner"
    )
}

#[test]
fn test_creare_swap_order() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("maker", &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            owner: Some("stepit".to_string()),
        },
    )
    .unwrap();

    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uatom"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: None,
        timeout: 10,
    };
    execute(deps.as_mut(), env.clone(), info.clone(), create_order_msg).unwrap();

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::SwapOrdersByMaker {
            maker: "maker".to_string(),
        },
    )
    .unwrap();
    let SwapOrdersByMakerResponse { orders } = from_json(res).unwrap();

    assert_eq!(orders.len(), 1, "expected one swap order in the store");
    assert_eq!(
        orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uatom"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: None,
            timeout: 10 + env.block.time.seconds(),
            status: OrderStatus::Open,
        },
        "expected a swap order with different values"
    );
}

#[test]
fn test_creare_swap_order_handling_errors() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("maker", &[]);

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            owner: Some("stepit".to_string()),
        },
    )
    .unwrap();

    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uatom"),
        coin_out: Coin::new(1_000, "uatom"),
        taker: None,
        timeout: 10,
    };
    let err = execute(deps.as_mut(), env.clone(), info.clone(), create_order_msg);

    assert_eq!(
        err.unwrap_err(),
        ContractError::SameDenomError {
            denom: "uatom".to_string()
        }
    );

    let info = mock_info("maker", &[Coin::new(1_000, "uosmo")]);
    let create_order_msg = ExecuteMsg::CreateSwapOrder {
        coin_in: Coin::new(1_000, "uatom"),
        coin_out: Coin::new(1_000, "uosmo"),
        taker: None,
        timeout: 10,
    };
    let err = execute(deps.as_mut(), env.clone(), info.clone(), create_order_msg);

    assert_eq!(
        err.unwrap_err(),
        ContractError::FundsError {
            accepted: 0,
            received: 1
        }
    );
}

#[test]
fn test_accept_swap_order() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("taker", &[Coin::new(1_000, "usdc")]);
    let taker_addr = Addr::unchecked("taker");
    let maker_addr = Addr::unchecked("maker");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            owner: Some("stepit".to_string()),
        },
    )
    .unwrap();

    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: None,
                timeout: 10 + env.block.time.seconds(),
                status: OrderStatus::Open,
            },
        )
        .unwrap();

    let create_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: "maker".to_string(),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), create_order_msg).unwrap();

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::SwapOrdersByMaker {
            maker: "maker".to_string(),
        },
    )
    .unwrap();
    let SwapOrdersByMakerResponse { orders } = from_json(res).unwrap();

    assert_eq!(
        orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uatom"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(taker_addr.clone()),
            timeout: 10 + env.block.time.seconds(),
            status: OrderStatus::Accepted,
        },
        "expect no errors when taker is None"
    );

    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: Some(taker_addr.clone()),
                timeout: 10 + env.block.time.seconds(),
                status: OrderStatus::Open,
            },
        )
        .unwrap();

    let create_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: "maker".to_string(),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), create_order_msg).unwrap();

    let res = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::SwapOrdersByMaker {
            maker: "maker".to_string(),
        },
    )
    .unwrap();
    let SwapOrdersByMakerResponse { orders } = from_json(res).unwrap();

    assert_eq!(
        orders[0].1,
        SwapOrder {
            coin_in: Coin::new(1_000, "uatom"),
            coin_out: Coin::new(1_000, "usdc"),
            taker: Some(Addr::unchecked("taker".to_string())),
            timeout: 10 + env.block.time.seconds(),
            status: OrderStatus::Accepted,
        },
        "expect no errors when sender is equal to specified taker"
    );

    let order_pointer = ORDER_POINTER.load(&deps.storage).unwrap();
    assert_eq!(
        order_pointer,
        OrderPointer {
            maker: maker_addr,
            taker: taker_addr,
            order_id: 0,
        }
    );
}

#[test]
fn test_accept_swap_order_handling_errors() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("taker", &[]);
    let maker_addr = Addr::unchecked("maker");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg {
            owner: Some("stepit".to_string()),
        },
    )
    .unwrap();

    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: None,
                timeout: 10 + env.block.time.seconds(),
                status: OrderStatus::Open,
            },
        )
        .unwrap();

    let create_order_msg = ExecuteMsg::AcceptSwapOrder {
        order_id: 0,
        maker: "maker".to_string(),
    };
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        create_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::FundsError {
            accepted: 1,
            received: 0
        },
        "expected error when accepting order without sending funds"
    );

    let info = mock_info("maker", &[Coin::new(1_000, "usdc")]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        create_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::SenderIsMaker {},
        "expected error when maker wants to accept own order"
    );

    let info = mock_info("taker", &[Coin::new(1_000, "uosmo")]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        create_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::WrongCoin {
            sent_denom: "uosmo".to_string(),
            sent_amount: 1_000,
            expected_denom: "usdc".to_string(),
            expected_amount: 1_000,
        },
        "expected error when sending coin different than requested"
    );

    let mut expiration_time = env.block.time.seconds() - 1;
    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: None,
                timeout: expiration_time,
                status: OrderStatus::Open,
            },
        )
        .unwrap();

    let info = mock_info("taker", &[Coin::new(1_000, "usdc")]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        create_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::SwapOrderNotAvailable {
            status: OrderStatus::Open.to_string(),
            expiration: expiration_time
        },
        "expected error when order is expired"
    );

    expiration_time = env.block.time.seconds() + 1;
    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: None,
                timeout: expiration_time,
                status: OrderStatus::Accepted,
            },
        )
        .unwrap();

    let info = mock_info("taker", &[Coin::new(1_000, "usdc")]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        create_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::SwapOrderNotAvailable {
            status: OrderStatus::Accepted.to_string(),
            expiration: expiration_time
        },
        "expected error when order is not open"
    );

    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr.clone(), 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: Some(maker_addr),
                timeout: expiration_time,
                status: OrderStatus::Open,
            },
        )
        .unwrap();

    let info = mock_info("taker", &[Coin::new(1_000, "usdc")]);
    let err = execute(deps.as_mut(), env.clone(), info.clone(), create_order_msg);

    assert_eq!(
        err.unwrap_err(),
        ContractError::Unauthorized {},
        "expected error when sender is not the specified taker"
    );
}

#[test]
fn test_confirm_swap_order() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("maker", &[Coin::new(1_000, "uatom")]);
    let taker_addr = Addr::unchecked("taker");
    let maker_addr = Addr::unchecked("maker");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg { owner: None },
    )
    .unwrap();

    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: Some(taker_addr.clone()),
                timeout: env.block.time.seconds() + 10,
                status: OrderStatus::Accepted,
            },
        )
        .unwrap();

    let confirm_order_msg = ExecuteMsg::ConfirmSwapOrder {
        order_id: 0,
        maker: "maker".to_string(),
    };
    execute(deps.as_mut(), env.clone(), info.clone(), confirm_order_msg).unwrap();

    let order_pointer = ORDER_POINTER.may_load(&deps.storage).unwrap();
    assert_eq!(order_pointer, None,);
}

#[test]
fn test_confirm_swap_order_handling_errors() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info("maker", &[]);
    let taker_addr = Addr::unchecked("taker");
    let maker_addr = Addr::unchecked("maker");

    instantiate(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        InstantiateMsg { owner: None },
    )
    .unwrap();

    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: Some(taker_addr.clone()),
                timeout: env.block.time.seconds() + 10,
                status: OrderStatus::Accepted,
            },
        )
        .unwrap();

    let confirm_order_msg = ExecuteMsg::ConfirmSwapOrder {
        order_id: 0,
        maker: "maker".to_string(),
    };
    let err = execute(deps.as_mut(), env.clone(), info.clone(), confirm_order_msg);
    assert_eq!(
        err.unwrap_err(),
        ContractError::FundsError {
            accepted: 1,
            received: 0
        }
    );

    let info = mock_info("taker", &[Coin::new(1_000, "usdc")]);
    let confirm_order_msg = ExecuteMsg::ConfirmSwapOrder {
        order_id: 0,
        maker: "maker".to_string(),
    };
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        confirm_order_msg.clone(),
    );
    assert_eq!(err.unwrap_err(), ContractError::Unauthorized,);

    let info = mock_info("maker", &[Coin::new(1_000, "uosmo")]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        confirm_order_msg.clone(),
    );
    assert_eq!(
        err.unwrap_err(),
        ContractError::WrongCoin {
            sent_denom: "uosmo".to_string(),
            sent_amount: 1_000,
            expected_denom: "uatom".to_string(),
            expected_amount: 1_000,
        },
        "expected error when sending coin different than requested"
    );

    let mut expiration_time = env.block.time.seconds() - 1;
    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: None,
                timeout: expiration_time,
                status: OrderStatus::Open,
            },
        )
        .unwrap();

    let info = mock_info("maker", &[Coin::new(1_000, "usdc")]);
    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        confirm_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::SwapOrderNotAvailable {
            status: OrderStatus::Open.to_string(),
            expiration: expiration_time
        },
        "expected error when order is expired"
    );

    expiration_time = env.block.time.seconds() + 1;
    SWAP_ORDERS
        .save(
            &mut deps.storage,
            (&maker_addr, 0),
            &SwapOrder {
                coin_in: Coin::new(1_000, "uatom"),
                coin_out: Coin::new(1_000, "usdc"),
                taker: None,
                timeout: expiration_time,
                status: OrderStatus::Confirmed,
            },
        )
        .unwrap();

    let err = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        confirm_order_msg.clone(),
    );

    assert_eq!(
        err.unwrap_err(),
        ContractError::SwapOrderNotAvailable {
            status: OrderStatus::Confirmed.to_string(),
            expiration: expiration_time
        },
        "expected error has been already confirmed"
    );
}

#[test]
fn test_replies() {
    let mut deps = mock_dependencies();

    let owner_addr = Addr::unchecked("0xowner".to_string());
    let maker_addr = Addr::unchecked("0xmaker".to_string());
    let taker_addr = Addr::unchecked("0xtaker".to_string());

    let config = Config { owner: owner_addr };
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    ORDER_POINTER
        .save(
            deps.as_mut().storage,
            &OrderPointer {
                order_id: 0,
                maker: maker_addr.clone(),
                taker: taker_addr.clone(),
            },
        )
        .unwrap();

    let swap_order = SwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: Some(taker_addr),
        timeout: 10,
        status: OrderStatus::Accepted,
    };
    SWAP_ORDERS
        .save(deps.as_mut().storage, (&maker_addr, 0), &swap_order)
        .unwrap();

    let reply_msg = Reply {
        id: CONFIRM_ORDER_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(MsgExecResponse { results: vec![] }.into()),
        }),
    };
    reply(deps.as_mut(), mock_env(), reply_msg).unwrap();

    let order_pointer = ORDER_POINTER.may_load(deps.as_ref().storage).unwrap();
    assert_eq!(order_pointer, None)
}

#[test]
fn test_replies_handling_errors() {
    let mut deps = mock_dependencies();

    let owner_addr = Addr::unchecked("0xowner".to_string());
    let maker_addr = Addr::unchecked("0xmaker".to_string());
    let taker_addr = Addr::unchecked("0xtaker".to_string());

    let config = Config { owner: owner_addr };
    CONFIG.save(deps.as_mut().storage, &config).unwrap();

    ORDER_POINTER
        .save(
            deps.as_mut().storage,
            &OrderPointer {
                order_id: 1,
                maker: maker_addr.clone(),
                taker: taker_addr.clone(),
            },
        )
        .unwrap();

    let swap_order = SwapOrder {
        coin_in: Coin::new(1_000, "uosmo"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: Some(taker_addr),
        timeout: 10,
        status: OrderStatus::Accepted,
    };
    SWAP_ORDERS
        .save(deps.as_mut().storage, (&maker_addr, 0), &swap_order)
        .unwrap();

    let reply_msg = Reply {
        id: CONFIRM_ORDER_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(MsgExecResponse { results: vec![] }.into()),
        }),
    };
    let err = reply(deps.as_mut(), mock_env(), reply_msg);

    assert_eq!(err.unwrap_err(), ContractError::Unauthorized)
}
// Utils test

#[test]
fn test_validate_coins_number() {
    let funds = vec![Coin {
        denom: "foo".to_string(),
        amount: Uint128::new(100),
    }];
    let result = utils::validate_coins_number(&funds, 1);
    assert!(result.is_ok());

    let funds = vec![
        Coin {
            denom: "foo".to_string(),
            amount: Uint128::new(100),
        },
        Coin {
            denom: "bar".to_string(),
            amount: Uint128::new(200),
        },
    ];
    let result = utils::validate_coins_number(&funds, 1);
    assert!(result.is_err());
}

#[test]
fn test_validate_different_denoms() {
    let result = utils::validate_different_denoms(&"uosmo".to_string(), &"uosmo".to_string());
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ContractError::SameDenomError {
            denom: "uosmo".to_string(),
        }
    );
    let result = utils::validate_different_denoms(&"uosmo".to_string(), &"uatom".to_string());
    assert!(result.is_ok());
}

#[test]
fn test_check_correct_coins() {
    let sent_coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    let expected_coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };

    let result = utils::check_correct_coins(&sent_coin, &expected_coin);
    assert!(result.is_ok());

    let sent_coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    let expected_coin = Coin {
        denom: "uatom".to_string(),
        amount: Uint128::new(1_000),
    };

    let result = utils::check_correct_coins(&sent_coin, &expected_coin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ContractError::WrongCoin {
            sent_denom: "uosmo".to_string(),
            sent_amount: 1_000_u128,
            expected_denom: "uatom".to_string(),
            expected_amount: 1_000_u128,
        }
    );

    let sent_coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(1_000),
    };
    let expected_coin = Coin {
        denom: "uosmo".to_string(),
        amount: Uint128::new(2_000),
    };

    let result = utils::check_correct_coins(&sent_coin, &expected_coin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ContractError::WrongCoin {
            sent_denom: "uosmo".to_string(),
            sent_amount: 1_000_u128,
            expected_denom: "uosmo".to_string(),
            expected_amount: 2_000_u128,
        }
    );
}

#[test]
fn test_validate_status_and_expiration() {
    let swap_order = SwapOrder {
        coin_in: Coin::new(1_000, "uatom"),
        coin_out: Coin::new(1_000, "usdc"),
        taker: Some(Addr::unchecked("taker".to_string())),
        timeout: 10,
        status: OrderStatus::Accepted,
    };

    let mut block_time = 9;
    let mut valid_status = OrderStatus::Accepted;

    let result =
        utils::validate_status_and_expiration(&swap_order, valid_status.clone(), block_time);

    assert!(result.is_ok());

    block_time = 11;
    let result = utils::validate_status_and_expiration(&swap_order, valid_status, block_time);
    assert_eq!(
        result.unwrap_err(),
        ContractError::SwapOrderNotAvailable {
            status: OrderStatus::Accepted.to_string(),
            expiration: 10,
        }
    );

    block_time = 10;
    valid_status = OrderStatus::Open;

    let result = utils::validate_status_and_expiration(&swap_order, valid_status, block_time);
    assert_eq!(
        result.unwrap_err(),
        ContractError::SwapOrderNotAvailable {
            status: OrderStatus::Accepted.to_string(),
            expiration: 10,
        }
    );
}
