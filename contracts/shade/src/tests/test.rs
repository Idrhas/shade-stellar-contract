#![cfg(test)]

use crate::shade::Shade;
use crate::shade::ShadeClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

#[test]
fn test_initialize() {
    let env = Env::default();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    assert_eq!(client.get_admin(), admin);
}

#[should_panic(expected = "HostError: Error(Contract, #2)")]
#[test]
fn test_initialize_twice() {
    let env = Env::default();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin);
}

#[should_panic(expected = "HostError: Error(Contract, #3)")]
#[test]
fn test_get_admin_not_initialized() {
    let env = Env::default();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);

    client.get_admin();
}
