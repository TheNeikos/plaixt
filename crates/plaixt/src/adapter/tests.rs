use trustfall::provider::check_adapter_invariants;

use super::Adapter;

#[tokio::test]
async fn adapter_satisfies_trustfall_invariants() {
    let schema = Adapter::schema();
    let adapter = Adapter::new(
        schema.clone(),
        vec![],
        [].into(),
        None,
        tokio::runtime::Handle::current(),
    );
    check_adapter_invariants(schema, adapter);
}
