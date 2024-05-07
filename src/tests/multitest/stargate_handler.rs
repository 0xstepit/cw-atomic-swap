use anyhow::Result as AnyResult;
use cosmwasm_schema::cw_serde;
use cosmwasm_schema::schemars::JsonSchema;
use cosmwasm_std::{Addr, Api, Binary, BlockInfo, CustomQuery, Storage};
// use cw_multi_test::error::AnyResult;
use cw_multi_test::{AppResponse, CosmosRouter, Stargate};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[cw_serde]
#[derive(Default)]
pub struct CustomStargate {}

impl Stargate for CustomStargate {
    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        type_url: String,
        value: Binary,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        // Err(anyhow::anyhow!("Error"))
        Ok(AppResponse::default())
    }
}
