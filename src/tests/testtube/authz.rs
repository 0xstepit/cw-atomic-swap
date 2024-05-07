use osmosis_std::types::cosmos::authz::v1beta1::{
    MsgExec, MsgExecResponse, MsgGrant, MsgGrantResponse, QueryGranteeGrantsRequest,
    QueryGranteeGrantsResponse, QueryGranterGrantsRequest, QueryGranterGrantsResponse,
    QueryGrantsRequest, QueryGrantsResponse,
};
use test_tube::{fn_execute, fn_query};

use test_tube::module::Module;
use test_tube::runner::Runner;

pub struct Authz<'a, R: Runner<'a>> {
    runner: &'a R,
}

impl<'a, R: Runner<'a>> Module<'a, R> for Authz<'a, R> {
    fn new(runner: &'a R) -> Self {
        Self { runner }
    }
}

impl<'a, R> Authz<'a, R>
where
    R: Runner<'a>,
{
    fn_execute! {
        pub exec: MsgExec["/cosmos.authz.v1beta1.MsgExec"] => MsgExecResponse
    }

    fn_execute! {
        pub grant: MsgGrant["/cosmos.authz.v1beta1.MsgGrant"] => MsgGrantResponse
    }

    fn_query! {
        pub query_grantee_grants ["/cosmos.authz.v1beta1.Query/GranteeGrants"]: QueryGranteeGrantsRequest => QueryGranteeGrantsResponse
    }

    fn_query! {
        pub query_granter_grants ["/cosmos.authz.v1beta1.Query/GranterGrants"]: QueryGranterGrantsRequest => QueryGranterGrantsResponse
    }

    fn_query! {
        pub query_grants ["/cosmos.authz.v1beta1.Query/Grants"]: QueryGrantsRequest => QueryGrantsResponse
    }
}
