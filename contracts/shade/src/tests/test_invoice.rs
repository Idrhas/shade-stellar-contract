#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::InvoiceStatus;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_test() -> (Env, ShadeClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

#[test]
fn test_create_and_get_invoice_success() {
    let (env, client, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 1000;

    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token);
    assert_eq!(invoice_id, 1);

    let invoice = client.get_invoice(&invoice_id);

    assert_eq!(invoice.id, 1);
    assert_eq!(invoice.merchant_id, 1);
    assert_eq!(invoice.amount, amount);
    assert_eq!(invoice.token, token);
    assert_eq!(invoice.description, description);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
}

#[should_panic(expected = "HostError: Error(Contract, #8)")]
#[test]
fn test_get_invoice_not_found() {
    let (_env, client, _admin) = setup_test();
    client.get_invoice(&999);
}

#[should_panic(expected = "HostError: Error(Contract, #1)")] // NotAuthorized
#[test]
fn test_create_invoice_unregistered_merchant() {
    let (env, client, _admin) = setup_test();

    let unregistered_merchant = Address::generate(&env);
    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 1000;

    client.create_invoice(&unregistered_merchant, &description, &amount, &token);
}

#[should_panic(expected = "HostError: Error(Contract, #7)")] // InvalidAmount
#[test]
fn test_create_invoice_invalid_amount() {
    let (env, client, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 0;

    client.create_invoice(&merchant, &description, &amount, &token);
}
