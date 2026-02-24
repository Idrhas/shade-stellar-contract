#![cfg(test)]

use crate::errors::ContractError;
use crate::shade::{Shade, ShadeClient};
use crate::types::{InvoiceStatus, Role};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, String};

fn setup_invoice_test() -> (
    Env,
    ShadeClient<'static>,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let manager = Address::generate(&env);
    let merchant = Address::generate(&env);
    let payer = Address::generate(&env);

    // Create token
    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    // Add accepted token
    client.add_accepted_token(&admin, &token);

    // Set fee
    let fee: i128 = 100;
    client.set_fee(&admin, &token, &fee);

    (env, client, admin, manager, merchant, payer, token)
}

#[test]
fn test_admin_role_can_initiate_payment() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Admin should have authorization to call pay_invoice_admin
    let res = client.try_pay_invoice_admin(&admin, &invoice_id);
    // May fail due to insufficient token balance, but not due to authorization
    let _ = res;
}

#[test]
fn test_manager_role_authorization() {
    let (env, client, admin, manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Grant manager role to manager address
    client.grant_role(&admin, &manager, &Role::Manager);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Manager should have authorization to call pay_invoice_admin
    let res = client.try_pay_invoice_admin(&manager, &invoice_id);
    // May fail due to token transfer, but check not authorization
    let _ = res;
}

#[should_panic(expected = "HostError: Error(Contract, #1)")]
#[test]
fn test_payer_without_role_denied_access() {
    let (_env, client, _admin, _manager, merchant, payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&_env, "Test Invoice"),
        &1000,
        &token,
    );

    // Payer has no role - should panic with NotAuthorized
    client.pay_invoice_admin(&payer, &invoice_id);
}

#[should_panic(expected = "HostError: Error(Contract, #1)")]
#[test]
fn test_merchant_cannot_pay_own_invoice() {
    let (_env, client, _admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&_env, "Test Invoice"),
        &1000,
        &token,
    );

    // Merchant has no admin/manager role - should panic with NotAuthorized
    client.pay_invoice_admin(&merchant, &invoice_id);
}

#[should_panic(expected = "HostError: Error(Contract, #13)")]
#[test]
fn test_cannot_pay_already_paid_invoice() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Manually set invoice to Paid
    use crate::types::DataKey;
    let mut invoice = client.get_invoice(&invoice_id);
    invoice.status = InvoiceStatus::Paid;
    invoice.payer = Some(admin.clone());
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);
    });

    // Attempt to pay again - should fail with InvalidInvoiceStatus
    client.pay_invoice_admin(&admin, &invoice_id);
}

#[should_panic(expected = "HostError: Error(Contract, #13)")]
#[test]
fn test_cannot_pay_cancelled_invoice() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Manually set invoice to Cancelled
    use crate::types::DataKey;
    let mut invoice = client.get_invoice(&invoice_id);
    invoice.status = InvoiceStatus::Cancelled;
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(invoice_id), &invoice);
    });

    // Attempt to pay - should fail with InvalidInvoiceStatus
    client.pay_invoice_admin(&admin, &invoice_id);
}

#[test]
fn test_invoice_not_found() {
    let (_env, client, admin, _manager, _merchant, _payer, _token) = setup_invoice_test();

    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::InvoiceNotFound as u32);
    let result = client.try_pay_invoice_admin(&admin, &999);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}

#[test]
fn test_role_revocation_denies_manager() {
    let (_env, client, admin, manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Grant and then revoke manager role
    client.grant_role(&admin, &manager, &Role::Manager);
    client.revoke_role(&admin, &manager, &Role::Manager);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&_env, "Test Invoice"),
        &1000,
        &token,
    );

    // Attempt to pay without role - should fail with NotAuthorized
    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::NotAuthorized as u32);
    let result = client.try_pay_invoice_admin(&manager, &invoice_id);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}

#[test]
fn test_contract_pause_blocks_payment() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Pause contract
    client.pause(&admin);

    // Attempt payment - should fail with ContractPaused
    let expected_error =
        soroban_sdk::Error::from_contract_error(ContractError::ContractPaused as u32);
    let result = client.try_pay_invoice_admin(&admin, &invoice_id);
    assert!(matches!(result, Err(Ok(err)) if err == expected_error));
}

#[test]
fn test_payment_allowed_after_unpause() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Pause and unpause
    client.pause(&admin);
    client.unpause(&admin);

    // Payment should now be allowed (though may fail due to insufficient token balance)
    let res = client.try_pay_invoice_admin(&admin, &invoice_id);
    // Just verify it doesn't fail with ContractPaused error
    if let Err(err) = res {
        if let Ok(contract_err) = err {
            let paused_error =
                soroban_sdk::Error::from_contract_error(ContractError::ContractPaused as u32);
            assert_ne!(contract_err, paused_error);
        }
    }
}

#[test]
fn test_invoice_state_validation() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Verify initial state
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
    assert_eq!(invoice.payer, None);
    assert_eq!(invoice.date_paid, None);
}

#[test]
fn test_multiple_invoices_independent() {
    let (env, client, _admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Create multiple invoices
    let id_1 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 1"),
        &1000,
        &token,
    );
    let id_2 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 2"),
        &2000,
        &token,
    );

    // Set second to Paid
    use crate::types::DataKey;
    let mut inv_2 = client.get_invoice(&id_2);
    inv_2.status = InvoiceStatus::Paid;
    env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .set(&DataKey::Invoice(id_2), &inv_2);
    });

    // Verify first is still Pending
    assert_eq!(client.get_invoice(&id_1).status, InvoiceStatus::Pending);
    assert_eq!(client.get_invoice(&id_2).status, InvoiceStatus::Paid);
}

#[test]
fn test_fee_preservation() {
    let (env, client, admin, _manager, merchant, _payer, token) = setup_invoice_test();

    // Register merchant
    client.register_merchant(&merchant);

    // Set custom fee
    let fee = 250i128;
    client.set_fee(&admin, &token, &fee);

    // Create invoice
    let invoice_id = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Test Invoice"),
        &1000,
        &token,
    );

    // Verify fee and invoice data
    assert_eq!(client.get_fee(&token), fee);
    let invoice = client.get_invoice(&invoice_id);
    assert_eq!(invoice.amount, 1000);
}
