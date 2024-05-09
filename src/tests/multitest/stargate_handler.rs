use anyhow::Result as AnyResult;
use cosmwasm_schema::cw_serde;
use cosmwasm_schema::schemars::JsonSchema;
use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CustomQuery, Storage};
use cw_multi_test::{AppResponse, CosmosRouter, Stargate};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[cw_serde]
#[derive(Default)]
pub struct CustomStargate {}

impl Stargate for CustomStargate {
    fn execute<ExecC, QueryC>(
        &self,
        __api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        _sender: Addr,
        _type_url: String,
        _value: Binary,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        // We trigger the error based on the block since at
        // this point the sender is always the contract.
        if block.height != 1 {
            return Ok(AppResponse::default());
        } else {
            return Err(anyhow::anyhow!("Failed to use auhtz"));
        }
    }
}
