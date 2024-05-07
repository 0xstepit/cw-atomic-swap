use std::{collections::HashMap, path::PathBuf};

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
};

use cosmwasm_std::{to_json_binary, Coin};
use osmosis_std::types::{
    cosmos::bank::v1beta1::QueryAllBalancesRequest,
    cosmwasm::wasm::v1::MsgExecuteContractResponse,
    osmosis::cosmwasmpool::v1beta1::{
        ContractInfoByPoolIdRequest, ContractInfoByPoolIdResponse, MsgCreateCosmWasmPool,
    },
};

use osmosis_test_tube::{
    Account, Bank, Module, OsmosisTestApp, RunnerError, RunnerExecuteResult, RunnerResult,
    SigningAccount, Wasm,
};
use serde::de::DeserializeOwned;

pub const WEEK: i64 = 7 * 24 * 60 * 60;

pub struct TestEnv<'a> {
    pub app: &'a OsmosisTestApp,
    pub owner: SigningAccount,
    pub contract: AtomicSwapContract<'a>,
    pub accounts: HashMap<String, SigningAccount>,
}

impl<'a> TestEnv<'a> {
    pub fn assert_account_balances(
        &self,
        account: &str,
        expected_balances: Vec<Coin>,
        ignore_denoms: Vec<&str>,
    ) {
        let account_balances: Vec<Coin> = Bank::new(self.app)
            .query_all_balances(&QueryAllBalancesRequest {
                address: self.accounts.get(account).unwrap().address(),
                pagination: None,
            })
            .unwrap()
            .balances
            .into_iter()
            .map(|coin| Coin::new(coin.amount.parse().unwrap(), coin.denom))
            .filter(|coin| !ignore_denoms.contains(&coin.denom.as_str()))
            .collect();

        assert_eq!(account_balances, expected_balances);
    }

    pub fn assert_contract_balances(&self, expected_balances: &[Coin]) {
        let contract_balances: Vec<Coin> = Bank::new(self.app)
            .query_all_balances(&QueryAllBalancesRequest {
                address: self.contract.contract_addr.clone(),
                pagination: None,
            })
            .unwrap()
            .balances
            .into_iter()
            .map(|coin| Coin::new(coin.amount.parse().unwrap(), coin.denom))
            .collect();

        assert_eq!(contract_balances, expected_balances);
    }
}

pub struct TestEnvBuilder {
    account_balances: HashMap<String, Vec<Coin>>,
    instantiate_msg: Option<InstantiateMsg>,
    admin: Option<String>,
}

impl TestEnvBuilder {
    pub fn new() -> Self {
        Self {
            account_balances: HashMap::new(),
            instantiate_msg: None,
            admin: None,
        }
    }

    /// Allows to set the init message for the contract.
    pub fn with_instantiate_msg(mut self, msg: InstantiateMsg) -> Self {
        self.instantiate_msg = Some(msg);
        self
    }

    /// Defines the admin for contract instantiation.
    pub fn with_admin(mut self, admin: &str) -> Self {
        self.admin = Some(admin.to_string());
        self
    }

    // Defines accounts and balances for the chain. Native Osmosis token will
    // be added by default.
    pub fn with_account(mut self, account: &str, balance: Vec<Coin>) -> Self {
        self.account_balances.insert(account.to_string(), balance);
        self
    }
    pub fn build(self, app: &'_ OsmosisTestApp) -> TestEnv<'_> {
        // Initialize all accounts in speicifed, if any. uosmo coins
        // will be added by default to each account.
        let accounts: HashMap<_, _> = self
            .account_balances
            .into_iter()
            .map(|(account, balance)| {
                let balance: Vec<_> = balance
                    .into_iter()
                    .chain(vec![Coin::new(1_000_000_000_000, "uosmo")])
                    .collect();

                (account, app.init_account(&balance).unwrap())
            })
            .collect();

        // Owner is the account that instantiate the contract.
        let owner = app
            .init_account(&[Coin::new(1_000_000_000_000_000_000u128, "uosmo")])
            .unwrap();

        // Add owner address to init message.
        let instantiate_msg = self.instantiate_msg.expect("instantiate msg not set");
        let instantiate_msg = InstantiateMsg {
            owner: accounts.get("owner").map(|admin| admin.address()),
            ..instantiate_msg
        };

        let contract =
            AtomicSwapContract::store_and_instantiate(app, &instantiate_msg, &owner).unwrap();

        TestEnv {
            app,
            owner,
            contract,
            accounts,
        }
    }
}

pub struct AtomicSwapContract<'a> {
    app: &'a OsmosisTestApp,
    pub code_id: u64,
    pub contract_addr: String,
}

impl<'a> AtomicSwapContract<'a> {
    pub fn new(app: &'a OsmosisTestApp, code_id: u64, contract_addr: String) -> Self {
        Self {
            app,
            code_id,
            contract_addr,
        }
    }

    /// Store and instantiate the atomic swap market.
    pub fn store_and_instantiate(
        app: &'a OsmosisTestApp,
        instantiate_msg: &InstantiateMsg,
        signer: &SigningAccount,
    ) -> Result<Self, RunnerError> {
        let wasm = Wasm::new(app);

        let wasm_byte_code = std::fs::read("./artifacts/cw_atomic_swap-aarch64.wasm").unwrap();
        // let wasm_byte_code =
        //     std::fs::read("./target/wasm32-unknown-unknown/release/cw_atomic_swap.wasm").unwrap();

        println!("Storing atomic swap byte code...");

        let code_id = wasm
            .store_code(&wasm_byte_code, None, signer)
            .unwrap()
            .data
            .code_id;

        println!("Instantiating atomic swap contract...");

        let market_address = wasm
            .instantiate(
                code_id,
                instantiate_msg,
                None,
                Some("Atomic swap market"),
                &[],
                signer,
            )
            .unwrap()
            .data
            .address;

        Ok(Self {
            app,
            code_id,
            contract_addr: market_address,
        })
    }

    /// Execute a smart contract call.
    pub fn execute(
        &self,
        msg: &ExecuteMsg,
        funds: &[Coin],
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        println!("Executing atomic swap contract...");

        let wasm = Wasm::new(self.app);
        wasm.execute(&self.contract_addr, msg, funds, signer)
    }

    /// Perform a wasm query against the smart contract state.
    pub fn query<Res>(&self, msg: &QueryMsg) -> RunnerResult<Res>
    where
        Res: ?Sized + DeserializeOwned,
    {
        let wasm = Wasm::new(self.app);
        wasm.query(&self.contract_addr, msg)
    }
}

pub fn assert_contract_err(expected: ContractError, actual: RunnerError) {
    match actual {
        RunnerError::ExecuteError { msg } => {
            if !msg.contains(&expected.to_string()) {
                panic!(
                    "assertion failed:\n\n  must contain \t: \"{}\",\n  actual \t: \"{}\"\n",
                    expected, msg
                )
            }
        }
        _ => panic!("unexpected error, expect execute error but got: {}", actual),
    };
}
