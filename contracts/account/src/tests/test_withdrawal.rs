#![cfg(test)]

use crate::account::MerchantAccount;
use crate::account::MerchantAccountClient;
use crate::events::WithdrawalToEvent;
use soroban_sdk::testutils::{Address as _, Events as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{token, Address, Env, IntoVal, Map, Symbol, TryFromVal, Val};

// ── Shared helpers ────────────────────────────────────────────────────────────

fn setup_initialized_account(env: &Env) -> (Address, MerchantAccountClient<'_>, Address, Address) {
    let contract_id = env.register(MerchantAccount, ());
    let client = MerchantAccountClient::new(env, &contract_id);

    let merchant = Address::generate(env);
    let manager = Address::generate(env);
    let merchant_id = 1u64;
    client.initialize(&merchant, &manager, &merchant_id);

    (contract_id, client, merchant, manager)
}

fn create_test_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin).address()
}

fn mint_to_contract(env: &Env, token: &Address, contract_id: &Address, amount: i128) {
    token::StellarAssetClient::new(env, token).mint(contract_id, &amount);
}

// ── 1. Successful external withdrawal ────────────────────────────────────────

#[test]
fn test_withdraw_to_success() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, client, _merchant, _manager) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    let initial_amount: i128 = 1_000;
    mint_to_contract(&env, &token, &contract_id, initial_amount);

    let withdraw_amount: i128 = 400;
    client.withdraw_to(&token, &withdraw_amount, &recipient);

    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&recipient), withdraw_amount);
    assert_eq!(
        token_client.balance(&contract_id),
        initial_amount - withdraw_amount
    );

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let expected_event = WithdrawalToEvent {
        token: token.clone(),
        recipient: recipient.clone(),
        amount: withdraw_amount,
        timestamp: env.ledger().timestamp(),
    };
    let emitted = events.get(0).unwrap();
    let emitted_data = Map::<Symbol, Val>::try_from_val(&env, &emitted.2).unwrap();
    let expected_data = Map::<Symbol, Val>::try_from_val(&env, &expected_event.data(&env)).unwrap();

    assert_eq!(emitted.0, contract_id);
    assert_eq!(emitted.1, expected_event.topics(&env));
    assert_eq!(emitted_data, expected_data);
}

// ── 2. Unauthorized caller ────────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_withdraw_to_unauthorized() {
    let env = Env::default();

    let (contract_id, client, _merchant, _manager) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);
    let random = Address::generate(&env);

    mint_to_contract(&env, &token, &contract_id, 1_000);

    client
        .mock_auths(&[MockAuth {
            address: &random,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "withdraw_to",
                args: (&token, &100_i128, &recipient).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .withdraw_to(&token, &100, &recipient);
}

// ── 3. Insufficient balance ───────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_withdraw_to_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, client, _merchant, _manager) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    mint_to_contract(&env, &token, &contract_id, 100);

    client.withdraw_to(&token, &999, &recipient);
}

// ── 4. Self-transfer ──────────────────────────────────────────────────────────

#[test]
fn test_withdraw_to_self_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, client, merchant, _manager) = setup_initialized_account(&env);
    let token = create_test_token(&env);

    let initial_amount: i128 = 500;
    mint_to_contract(&env, &token, &contract_id, initial_amount);

    let withdraw_amount: i128 = 200;
    client.withdraw_to(&token, &withdraw_amount, &merchant);

    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&merchant), withdraw_amount);
    assert_eq!(
        token_client.balance(&contract_id),
        initial_amount - withdraw_amount
    );
}

// ── 5. Manager cannot withdraw ────────────────────────────────────────────────

#[test]
#[should_panic]
fn test_withdraw_to_manager_cannot_withdraw() {
    let env = Env::default();

    let (contract_id, client, _merchant, manager) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    mint_to_contract(&env, &token, &contract_id, 1_000);

    client
        .mock_auths(&[MockAuth {
            address: &manager,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "withdraw_to",
                args: (&token, &100_i128, &recipient).into_val(&env),
                sub_invokes: &[],
            },
        }])
        .withdraw_to(&token, &100, &recipient);
}

// ── 6. Failed withdrawal leaves balances unchanged ───────────────────────────

#[test]
fn test_withdraw_to_insufficient_balance_leaves_contract_unchanged() {
    let env = Env::default();
    env.mock_all_auths();

    let (contract_id, client, _merchant, _manager) = setup_initialized_account(&env);
    let token = create_test_token(&env);
    let recipient = Address::generate(&env);

    let initial_amount: i128 = 100;
    mint_to_contract(&env, &token, &contract_id, initial_amount);

    let result = std::panic::catch_unwind(|| {
        client.withdraw_to(&token, &999, &recipient);
    });

    assert!(result.is_err());

    let token_client = token::TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&contract_id), initial_amount);
    assert_eq!(token_client.balance(&recipient), 0);
}