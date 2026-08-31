#![cfg(test)]
extern crate alloc;

use super::*;
use alloc::string::String as StdString;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, BytesN, Env, String,
};

fn setup_token(env: &Env, admin: &Address) -> Address {
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    StellarAssetClient::new(env, &token_id.address()).mint(admin, &10_000);
    token_id.address()
}

fn setup_contract(env: &Env) -> (LinkoraContractClient<'_>, Address, Address) {
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let treasury = Address::generate(env);
    client.initialize(&admin, &treasury, &0);
    (client, admin, treasury)
}

fn upload_upgrade_wasm(env: &Env) -> BytesN<32> {
    let wasm = Bytes::from_slice(env, include_bytes!("../linkora_contracts.wasm"));
    env.deployer().upload_contract_wasm(wasm)
}

#[test]
fn test_set_and_get_profile() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, String::from_str(&env, "alice"));
}

#[test]
fn test_username_reverse_index_registration() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);

    let resolved = client.get_address_by_username(&String::from_str(&env, "alice"));
    assert_eq!(resolved, Some(user));
}

#[test]
fn test_username_reverse_index_update() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    client.set_profile(&user, &String::from_str(&env, "alice2"), &token);

    // Old username should be gone
    assert!(client
        .get_address_by_username(&String::from_str(&env, "alice"))
        .is_none());
    // New username should resolve
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice2")),
        Some(user)
    );
}

// ── Issue #714: get_address_by_username returns None for unregistered username ─

#[test]
fn test_get_address_by_username_returns_none_for_unregistered() {
    // Call get_address_by_username('unknown') on a fresh contract.
    // Verify it returns None without panicking.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let result = client.get_address_by_username(&String::from_str(&env, "unknown"));
    assert_eq!(
        result, None,
        "get_address_by_username must return None for a username that was never registered"
    );
}

#[test]
#[should_panic(expected = "username taken")]
fn test_username_duplicate_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_profile(&user1, &String::from_str(&env, "shared_username"), &token);
    client.set_profile(&user2, &String::from_str(&env, "shared_username"), &token);
}

// ── Pagination tests ──────────────────────────────────────────────────────────

#[test]
fn test_get_following_first_page() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let mut followees = soroban_sdk::vec![&env];
    for _ in 0..10 {
        followees.push_back(Address::generate(&env));
    }

    for followee in followees.iter() {
        client.follow(&alice, &followee);
    }

    let page = client.get_following(&alice, &0, &5);
    assert_eq!(page.len(), 5);
    assert_eq!(page.get(0).unwrap(), followees.get(0).unwrap());
    assert_eq!(page.get(4).unwrap(), followees.get(4).unwrap());
}

#[test]
fn test_get_following_second_page() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let mut followees = soroban_sdk::vec![&env];
    for _ in 0..10 {
        followees.push_back(Address::generate(&env));
    }

    for followee in followees.iter() {
        client.follow(&alice, &followee);
    }

    let page = client.get_following(&alice, &5, &5);
    assert_eq!(page.len(), 5);
    assert_eq!(page.get(0).unwrap(), followees.get(5).unwrap());
    assert_eq!(page.get(4).unwrap(), followees.get(9).unwrap());
}

#[test]
fn test_get_following_offset_beyond_end() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.follow(&alice, &bob);

    let page = client.get_following(&alice, &10, &10);
    assert_eq!(page.len(), 0);
}

#[test]
#[should_panic(expected = "limit must be between 1 and 50")]
fn test_get_following_limit_exceeds_maximum() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.follow(&alice, &bob);

    client.get_following(&alice, &0, &51);
}

#[test]
#[should_panic(expected = "limit must be between 1 and 50")]
fn test_get_following_zero_limit() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.follow(&alice, &bob);

    client.get_following(&alice, &0, &0);
}

#[test]
fn test_get_posts_by_author_first_page() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    for i in 0..10 {
        let post_str = if i == 0 {
            String::from_str(&env, "post 0")
        } else if i == 1 {
            String::from_str(&env, "post 1")
        } else if i == 2 {
            String::from_str(&env, "post 2")
        } else if i == 3 {
            String::from_str(&env, "post 3")
        } else if i == 4 {
            String::from_str(&env, "post 4")
        } else if i == 5 {
            String::from_str(&env, "post 5")
        } else if i == 6 {
            String::from_str(&env, "post 6")
        } else if i == 7 {
            String::from_str(&env, "post 7")
        } else if i == 8 {
            String::from_str(&env, "post 8")
        } else {
            String::from_str(&env, "post 9")
        };
        client.create_post(&author, &post_str);
    }

    let page = client.get_posts_by_author(&author, &0, &5);
    assert_eq!(page.len(), 5);
    assert_eq!(page.get(0).unwrap(), 1u64);
    assert_eq!(page.get(4).unwrap(), 5u64);
}

#[test]
fn test_get_posts_by_author_second_page() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    for i in 0..10 {
        let post_str = if i == 0 {
            String::from_str(&env, "post 0")
        } else if i == 1 {
            String::from_str(&env, "post 1")
        } else if i == 2 {
            String::from_str(&env, "post 2")
        } else if i == 3 {
            String::from_str(&env, "post 3")
        } else if i == 4 {
            String::from_str(&env, "post 4")
        } else if i == 5 {
            String::from_str(&env, "post 5")
        } else if i == 6 {
            String::from_str(&env, "post 6")
        } else if i == 7 {
            String::from_str(&env, "post 7")
        } else if i == 8 {
            String::from_str(&env, "post 8")
        } else {
            String::from_str(&env, "post 9")
        };
        client.create_post(&author, &post_str);
    }

    let page = client.get_posts_by_author(&author, &5, &5);
    assert_eq!(page.len(), 5);
    assert_eq!(page.get(0).unwrap(), 6u64);
    assert_eq!(page.get(4).unwrap(), 10u64);
}

#[test]
fn test_get_posts_by_author_offset_beyond_end() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    client.create_post(&author, &String::from_str(&env, "post 1"));

    let page = client.get_posts_by_author(&author, &10, &10);
    assert_eq!(page.len(), 0);
}

#[test]
#[should_panic(expected = "limit must be between 1 and 50")]
fn test_get_posts_by_author_limit_exceeds_maximum() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    client.create_post(&author, &String::from_str(&env, "post 1"));

    client.get_posts_by_author(&author, &0, &51);
}

#[test]
fn test_get_posts_by_author_offset_beyond_list_length_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    for i in 0..5 {
        client.create_post(
            &author,
            &String::from_str(&env, &alloc::format!("post {i}")),
        );
    }

    // Offset is past the end of the 5-item list.
    let page = client.get_posts_by_author(&author, &5, &10);
    assert_eq!(page.len(), 0);

    let page_far = client.get_posts_by_author(&author, &100, &10);
    assert_eq!(page_far.len(), 0);
}

#[test]
fn test_get_posts_by_author_offset_plus_limit_beyond_end_returns_remaining() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    let mut ids = Vec::new(&env);
    for i in 0..10 {
        ids.push_back(client.create_post(
            &author,
            &String::from_str(&env, &alloc::format!("post {i}")),
        ));
    }

    // offset (8) + limit (10) = 18, which is beyond the 10-item list,
    // so only the 2 remaining items should be returned.
    let page = client.get_posts_by_author(&author, &8, &10);
    assert_eq!(page.len(), 2);
    assert_eq!(page.get(0).unwrap(), ids.get(8).unwrap());
    assert_eq!(page.get(1).unwrap(), ids.get(9).unwrap());
}

#[test]
fn test_get_posts_by_author_limit_50_max_allowed_returns_all() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    for i in 0..50 {
        client.create_post(
            &author,
            &String::from_str(&env, &alloc::format!("post {i}")),
        );
    }

    // limit = 50 is the maximum allowed value and must not panic.
    let page = client.get_posts_by_author(&author, &0, &50);
    assert_eq!(page.len(), 50);
}

#[test]
fn test_get_posts_by_author_after_delete() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let author = Address::generate(&env);

    let id1 = client.create_post(&author, &String::from_str(&env, "post 1"));
    let id2 = client.create_post(&author, &String::from_str(&env, "post 2"));
    let id3 = client.create_post(&author, &String::from_str(&env, "post 3"));

    // Delete middle post
    client.delete_post(&author, &id2);

    let page = client.get_posts_by_author(&author, &0, &10);
    assert_eq!(page.len(), 2);
    assert_eq!(page.get(0).unwrap(), id1);
    assert_eq!(page.get(1).unwrap(), id3);
}

// ── Post tests ────────────────────────────────────────────────────────────────

#[test]
fn test_username_same_user_can_reregister_same_name() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    // Same user re-registering with the same username should not panic
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice")),
        Some(user)
    );
}

#[test]
fn test_tip_fee_split() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    // Initialize with 2.5% fee (250 bps)
    client.initialize(&admin, &treasury, &250);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "Fee test post"));

    // Tip 1000 units
    client.tip(&tipper, &post_id, &token, &1000);

    // Verify balances
    // Fee = 1000 * 250 / 10000 = 25
    // Author gets 1000 - 25 = 975
    assert_eq!(TokenClient::new(&env, &token).balance(&treasury), 25);
    assert_eq!(TokenClient::new(&env, &token).balance(&author), 975);

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.tip_total, 975);
}

#[test]
#[should_panic(expected = "blocked")]
fn test_tip_blocked_by_author() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &250);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "Test post"));

    // Author blocks tipper
    client.block_user(&author, &tipper);

    // Tipper tries to tip - should panic with "blocked"
    client.tip(&tipper, &post_id, &token, &1000);
}

#[test]
fn test_tip_after_unblock() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &250);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "Test post"));

    // Author blocks tipper
    client.block_user(&author, &tipper);

    // Author unblocks tipper
    client.unblock_user(&author, &tipper);

    // Tipper can now tip successfully
    client.tip(&tipper, &post_id, &token, &1000);

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.tip_total, 975);
}

#[test]
fn test_tip_non_blocked_user() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper1 = Address::generate(&env);
    let tipper2 = Address::generate(&env);

    client.initialize(&admin, &treasury, &250);

    let token = setup_token(&env, &tipper1);
    StellarAssetClient::new(&env, &token).mint(&tipper2, &5000);

    let post_id = client.create_post(&author, &String::from_str(&env, "Test post"));

    // Author blocks tipper1
    client.block_user(&author, &tipper1);

    // Tipper2 (not blocked) can tip successfully
    client.tip(&tipper2, &post_id, &token, &500);

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.tip_total, 488);
}

// ── Issue #722: strengthen coverage that block_user prevents tipping ──────
//
// The existing test_tip_blocked_by_author asserts only that the call panics
// with "blocked". These three tests pin down additional invariants and edge
// cases that the basic test does not directly assert.

// (A) A blocked tip attempt must leave *no* half-committed state. The panic
//     must fire before any token movement, fee accounting, tip_total update,
//     or cooldown write. Use try_tip so we can inspect post-state after the
//     failed call.
#[test]
fn test_tip_block_preserves_no_state_changes_on_panic() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &250);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "block-prevented tip post"));

    // Author blocks tipper.
    client.block_user(&author, &tipper);

    // Snapshot every pre-state we care about so the post-state assertions are
    // explicit (rather than just trusting Soroban's try_* rollback semantics).
    let token_client = TokenClient::new(&env, &token);
    let tipper_balance_before = token_client.balance(&tipper);
    let author_balance_before = token_client.balance(&author);
    let treasury_balance_before = token_client.balance(&treasury);
    let post_before = client.get_post(&post_id).unwrap();
    let tip_total_before = post_before.tip_total;

    // Blocked tip attempt — must be rejected.
    let result = client.try_tip(&tipper, &post_id, &token, &1_000);
    assert!(result.is_err(), "blocked tip must return Err");

    // No state changes must have been committed by the failed call frame.
    assert_eq!(
        token_client.balance(&tipper),
        tipper_balance_before,
        "tipper balance must be unchanged after a blocked tip"
    );
    assert_eq!(
        token_client.balance(&author),
        author_balance_before,
        "author balance must be unchanged after a blocked tip"
    );
    assert_eq!(
        token_client.balance(&treasury),
        treasury_balance_before,
        "treasury must not collect a fee for a blocked tip"
    );
    assert_eq!(
        client.get_post(&post_id).unwrap().tip_total,
        tip_total_before,
        "post.tip_total must not be incremented for a blocked tip"
    );
    // The blocked relationship must remain intact (the block map must not be
    // corrupted by the failed attempt).
    assert!(
        client.is_blocked(&author, &tipper),
        "blocked relationship must persist after a rejected tip attempt"
    );

    // End-to-end cooldown-not-consumed check: a blocked tip attempt must NOT
    // burn the per-(post, tipper) cooldown. The `tip` function panics with
    // "blocked" *before* writing the TipCooldown key, so an unblocked
    // re-attempt on the same ledger must succeed. If the blocked attempt had
    // written the cooldown, this would panic with "tip cooldown not expired"
    // (the default TIP_COOLDOWN_LEDGERS is 17,280).
    //
    // This invariant is what makes test_tip_after_unblock work, but no
    // existing test pins it down directly.
    client.unblock_user(&author, &tipper);
    client.tip(&tipper, &post_id, &token, &1);

    let post_after_unblock = client.get_post(&post_id).unwrap();
    assert_eq!(
        post_after_unblock.tip_total, 1,
        "tip after a previously-blocked-then-unblocked attempt must succeed"
    );
}

// (B) Blocking must be unidirectional. If A blocks B (B is restricted), the
//     block must NOT also restrict A's interactions toward B — A must still
//     be able to tip B's posts and B must still receive that tip income.
//     This guards against an accidentally-symmetric block implementation.
#[test]
fn test_tip_block_is_unidirectional_blocker_can_still_tip_blocked() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let blocked_user = Address::generate(&env); // "B" — restricted by blocker
    let blocker = Address::generate(&env); // "A" — the blocker

    client.initialize(&admin, &treasury, &250);

    // A blocks B (B is restricted).
    client.block_user(&blocker, &blocked_user);

    // `blocker` holds tokens; mint extra to `blocked_user` so B can also
    // receive tips on B's own post.
    let token = setup_token(&env, &blocker);
    StellarAssetClient::new(&env, &token).mint(&blocked_user, &10_000);

    let post_id = client.create_post(
        &blocked_user,
        &String::from_str(&env, "blocked_user is the author here"),
    );

    // The blocker (A) tips the blocked_user's (B) post — must succeed because
    // the contract checks is_blocked(post.author=blocked_user, tipper=blocker),
    // and blocked_user has not blocked anyone.
    client.tip(&blocker, &post_id, &token, &1_000);

    // Standard 2.5% fee: fee = 25, author gets 975.
    let token_client = TokenClient::new(&env, &token);
    assert_eq!(
        token_client.balance(&treasury),
        25,
        "treasury receives the fee on the blocker's tip"
    );
    assert_eq!(
        token_client.balance(&blocked_user),
        10_975,
        "blocked_user (the author) still receives their share of the tip (10_000 minted at setup + 975 tip share)"
    );

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(
        post.tip_total, 975,
        "tip_total accumulates even though blocked_user is on someone else's block list"
    );
}

// (C) Author blocks two distinct addresses. Each blocked tipper's tip must
//     panic independently (no overwriting on the second block_user call, no
//     shared state between entries in the block map). An unrelated unblocked
//     tipper must still be able to tip the same post.
#[test]
fn test_tip_block_multiple_blocked_tippers_panic_independently() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let blocked_a = Address::generate(&env);
    let blocked_b = Address::generate(&env);
    let unblocked = Address::generate(&env);

    client.initialize(&admin, &treasury, &250);

    // `setup_token` mints 10,000 to `blocked_a`; mint extra to the other
    // addresses so they all have funds to tip.
    let token = setup_token(&env, &blocked_a);
    StellarAssetClient::new(&env, &token).mint(&blocked_b, &5_000);
    StellarAssetClient::new(&env, &token).mint(&unblocked, &10_000);

    let post_id = client.create_post(&author, &String::from_str(&env, "multi-block post"));

    // Author blocks two different addresses.
    client.block_user(&author, &blocked_a);
    client.block_user(&author, &blocked_b);

    // Each blocked user independently fails on the same post.
    let r_a = client.try_tip(&blocked_a, &post_id, &token, &1_000);
    assert!(r_a.is_err(), "first blocked tipper must be rejected");
    let r_b = client.try_tip(&blocked_b, &post_id, &token, &1_000);
    assert!(r_b.is_err(), "second blocked tipper must be rejected");

    // An unrelated, unblocked tipper succeeds and pays the fee.
    client.tip(&unblocked, &post_id, &token, &500);

    // Only the unblocked tipper's contribution is recorded.
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(
        post.tip_total, 488,
        "only the unblocked tipper's tip contributes to tip_total"
    );

    // No state mutations from the blocked attempts.
    let token_client = TokenClient::new(&env, &token);
    assert_eq!(
        token_client.balance(&blocked_a),
        10_000,
        "first blocked tipper's balance must be untouched"
    );
    assert_eq!(
        token_client.balance(&blocked_b),
        5_000,
        "second blocked tipper's balance must be untouched"
    );
    // Unblocked tipper pays the full tip amount; treasury + author split:
    //   fee = 500 * 250 / 10_000 = 12  (i128 integer division truncates)
    //   author receives 500 - 12 = 488
    let fee: i128 = 12;
    let author_amount: i128 = 488;
    assert_eq!(
        token_client.balance(&unblocked),
        10_000 - 500,
        "unblocked tipper pays the full tip amount"
    );
    assert_eq!(
        token_client.balance(&treasury),
        fee,
        "treasury receives the fee from the unblocked tipper only"
    );
    assert_eq!(
        token_client.balance(&author),
        author_amount,
        "author receives their share from the unblocked tipper only"
    );
}

#[test]
fn test_profile_count() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_profile(&user1, &String::from_str(&env, "alice"), &token);
    assert_eq!(client.get_profile_count(), 1);

    // Update profile should not increment count
    client.set_profile(&user1, &String::from_str(&env, "alice_new"), &token);
    assert_eq!(client.get_profile_count(), 1);

    client.set_profile(&user2, &String::from_str(&env, "bob"), &token);
    assert_eq!(client.get_profile_count(), 2);
}

#[test]
fn test_post_count() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    client.create_post(&author, &String::from_str(&env, "Post 1"));
    client.create_post(&author, &String::from_str(&env, "Post 2"));

    assert_eq!(client.get_post_count(), 2);
}

#[test]
fn test_post_count_not_decremented_on_delete() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id1 = client.create_post(&author, &String::from_str(&env, "Post 1"));
    let post_id2 = client.create_post(&author, &String::from_str(&env, "Post 2"));

    assert_eq!(client.get_post_count(), 2);

    // Delete first post
    client.delete_post(&author, &post_id1);

    // Counter should still be 2 (total ever created)
    assert_eq!(client.get_post_count(), 2);

    // But the post should be gone
    assert!(client.get_post(&post_id1).is_none());
    assert!(client.get_post(&post_id2).is_some());
}

#[test]
fn test_get_post_returns_none_for_deleted_post() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Test Post"));

    assert_eq!(client.get_post_count(), 1);
    assert!(client.get_post(&post_id).is_some());

    // Delete the post
    client.delete_post(&author, &post_id);

    // Verify it returns None and get_post_count is unchanged
    assert!(client.get_post(&post_id).is_none());
    assert_eq!(client.get_post_count(), 1);
}

#[test]
fn test_follow_and_unfollow() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.follow(&alice, &bob);
    assert_eq!(client.get_following(&alice, &0, &10).len(), 1);
    assert_eq!(client.get_followers(&bob, &0, &10).len(), 1);

    client.unfollow(&alice, &bob);
    assert_eq!(client.get_following(&alice, &0, &10).len(), 0);
    assert_eq!(client.get_followers(&bob, &0, &10).len(), 0);
}

#[test]
fn test_block_prevents_follow() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let blocker = Address::generate(&env);
    let blocked = Address::generate(&env);
    client.block_user(&blocker, &blocked);
    assert!(client.is_blocked(&blocker, &blocked));
}

#[test]
#[should_panic(expected = "blocked")]
fn test_blocked_follow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Bob blocks Alice
    client.block_user(&bob, &alice);

    // Alice tries to follow Bob
    client.follow(&alice, &bob);
}

#[test]
fn test_blocked_user_cannot_follow_blocker_no_relationship_created() {
    // After block_user(A, B), follow(B, A) must panic with "blocked" and
    // must not create a follow relationship between B and A.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let a = Address::generate(&env);
    let b = Address::generate(&env);

    // A blocks B
    client.block_user(&a, &b);

    // B tries to follow A — must panic with "blocked"
    let result = client.try_follow(&b, &a);
    assert!(result.is_err());

    // No follow relationship was created in either direction
    assert_eq!(client.get_following(&b, &0, &50).len(), 0);
    assert_eq!(client.get_followers(&a, &0, &50).len(), 0);
}

#[test]
fn test_like_post() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let user = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Like test"));

    client.like_post(&user, &post_id);
    assert_eq!(client.get_like_count(&post_id), 1);
    assert!(client.has_liked(&user, &post_id));

    // Duplicate like should not increment
    client.like_post(&user, &post_id);
    assert_eq!(client.get_like_count(&post_id), 1);
}

#[test]
fn test_like_post_emits_event_on_first_like() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let user = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Event test"));

    client.like_post(&user, &post_id);

    assert!(
        !env.events().all().events().is_empty(),
        "LikePostEvent should be emitted"
    );
}

#[test]
fn test_like_post_no_event_on_duplicate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Duplicate event test"));

    client.like_post(&user1, &post_id);
    let like_count_after_first = client.get_like_count(&post_id);

    client.like_post(&user1, &post_id);
    let like_count_after_duplicate = client.get_like_count(&post_id);

    assert_eq!(
        like_count_after_duplicate, like_count_after_first,
        "duplicate like should not increment count"
    );

    client.like_post(&user2, &post_id);
    let like_count_after_new_user = client.get_like_count(&post_id);

    assert_eq!(
        like_count_after_new_user,
        like_count_after_first + 1,
        "like from new user should increment"
    );
}

// ── Issue #712: get_like_count returns 0 for post with no likes ───────────────

#[test]
fn test_get_like_count_returns_zero_for_post_with_no_likes() {
    // Create a post and immediately call get_like_count.
    // Verify it returns 0 without error.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "No likes yet"));

    assert_eq!(
        client.get_like_count(&post_id),
        0,
        "get_like_count must return 0 for a newly created post with no likes"
    );
}

#[test]
fn test_pool_authorization() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let other_user = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    // Give other_user some tokens to deposit
    StellarAssetClient::new(&env, &token).mint(&other_user, &1000);

    let pool_id = symbol_short!("pool1");
    // Create pool with 2-of-2 threshold
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    // Deposit works for anyone with tokens
    client.pool_deposit(&other_user, &pool_id, &token, &100);

    // Verify pool balance was updated
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 100);

    // Withdrawal by both admins works
    client.pool_withdraw(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &50,
        &other_user,
    );
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 50);
}

#[test]
fn test_create_pool_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin = Address::generate(&env);
    let token = setup_token(&env, &pool_admin);

    let pool_id = symbol_short!("pool_evt");

    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin.clone()],
        &1,
    );
    assert!(
        !env.events().all().events().is_empty(),
        "PoolCreatedEvent should be emitted"
    );
}

#[test]
#[should_panic(expected = "pool exists")]
fn test_create_pool_duplicate_id_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin = Address::generate(&env);
    let token = setup_token(&env, &pool_admin);

    let pool_id = symbol_short!("test");
    // First call should succeed
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin.clone()],
        &1,
    );
    // Second call with same id should panic with "pool exists"
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin.clone()],
        &1,
    );
}

#[test]
#[should_panic(expected = "insufficient signers")]
fn test_pool_withdraw_insufficient_signers() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let other_user = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);
    StellarAssetClient::new(&env, &token).mint(&other_user, &1000);

    let pool_id = symbol_short!("pool1");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );
    client.pool_deposit(&other_user, &pool_id, &token, &100);

    // Only 1 signer when 2 required
    client.pool_withdraw(&vec![&env, pool_admin1.clone()], &pool_id, &50, &other_user);
}

#[test]
#[should_panic(expected = "unauthorized signer")]
fn test_pool_withdraw_unauthorized_signer() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let unauthorized_user = Address::generate(&env);
    let other_user = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);
    StellarAssetClient::new(&env, &token).mint(&other_user, &1000);

    let pool_id = symbol_short!("pool2");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );
    client.pool_deposit(&other_user, &pool_id, &token, &100);

    // Try to withdraw with a signer not in pool.admins
    client.pool_withdraw(
        &vec![&env, pool_admin1.clone(), unauthorized_user.clone()],
        &pool_id,
        &50,
        &other_user,
    );
}

#[test]
#[should_panic(expected = "low balance")]
fn test_pool_withdraw_exceeds_balance() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let other_user = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);
    StellarAssetClient::new(&env, &token).mint(&other_user, &1000);

    let pool_id = symbol_short!("pool3");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &1,
    );
    client.pool_deposit(&other_user, &pool_id, &token, &100);

    // Try to withdraw more than available balance
    client.pool_withdraw(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &200,
        &other_user,
    );
}

#[test]
#[should_panic(expected = "wrong token for pool")]
fn test_pool_deposit_wrong_token_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin = Address::generate(&env);
    let other_user = Address::generate(&env);
    let correct_token = setup_token(&env, &pool_admin);
    let wrong_token = setup_token(&env, &pool_admin);

    // Give other_user some wrong tokens
    StellarAssetClient::new(&env, &wrong_token).mint(&other_user, &1000);

    let pool_id = symbol_short!("pool4");
    // Create pool with correct_token
    client.create_pool(
        &admin,
        &pool_id,
        &correct_token,
        &vec![&env, pool_admin.clone()],
        &1,
    );

    // Try to deposit with wrong_token - should panic
    client.pool_deposit(&other_user, &pool_id, &wrong_token, &100);
}

#[test]
fn test_pool_deposit_correct_token_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin = Address::generate(&env);
    let other_user = Address::generate(&env);
    let token = setup_token(&env, &pool_admin);

    // Give other_user some tokens to deposit
    StellarAssetClient::new(&env, &token).mint(&other_user, &1000);

    let pool_id = symbol_short!("pool5");
    // Create pool with the matching token
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin.clone()],
        &1,
    );

    // Pool starts empty
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 0);

    // Depositing with the correct token succeeds
    client.pool_deposit(&other_user, &pool_id, &token, &100);

    // Pool balance is updated and tokens were transferred into the contract
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 100);
    assert_eq!(TokenClient::new(&env, &token).balance(&other_user), 900);

    // A second deposit accumulates on the existing balance
    client.pool_deposit(&other_user, &pool_id, &token, &50);
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 150);
}

#[test]
fn test_sequential_posts() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);

    // Set first timestamp
    let ts1 = 1000;
    env.ledger().set_timestamp(ts1);

    // Create first post
    let post_id1 = client.create_post(&author, &String::from_str(&env, "First post"));
    assert_eq!(post_id1, 1);

    let post1 = client.get_post(&post_id1).unwrap();
    assert_eq!(post1.timestamp, ts1);
    assert_eq!(post1.id, 1);

    // Advance timestamp
    let ts2 = 2000;
    env.ledger().set_timestamp(ts2);

    // Create second post
    let post_id2 = client.create_post(&author, &String::from_str(&env, "Second post"));
    assert_eq!(post_id2, 2);

    let post2 = client.get_post(&post_id2).unwrap();
    assert_eq!(post2.timestamp, ts2);
    assert_eq!(post2.id, 2);
}

#[test]
#[should_panic(expected = "post does not exist: 999")]
fn test_delete_post_non_existent() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    client.delete_post(&author, &999);
}

// ── initialize / upgrade tests ────────────────────────────────────────────────

#[test]
fn test_initialize_stores_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);

    // Admin is stored: set_fee (admin-only) should succeed when called by admin
    client.set_fee(&admin, &100);
}

#[test]
fn test_initialize_stores_contract_state_version() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);

    let state: ContractState = env
        .as_contract(&client.address, || {
            env.storage().instance().get(&CONTRACT_STATE)
        })
        .expect("contract state should be initialized");
    assert_eq!(state.version, 1);
    assert_eq!(state.implementation_wasm_hash, None);
}

#[test]
#[should_panic(expected = "username must be at most 50 characters")]
fn test_set_profile_rejects_oversized_username() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let long_username = StdString::from_iter(core::iter::repeat_n('a', 51));
    client.set_profile(&user, &String::from_str(&env, &long_username), &token);
}

#[test]
#[should_panic(expected = "user must not be the zero address")]
fn test_set_profile_rejects_zero_address() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let zero_user = Address::from_str(
        &env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let token = Address::generate(&env);

    client.set_profile(&zero_user, &String::from_str(&env, "alice"), &token);
}

#[test]
#[should_panic(expected = "content must be at most 2000 characters")]
fn test_create_post_rejects_oversized_content() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let long_content = StdString::from_iter(core::iter::repeat_n('x', 2001));
    client.create_post(&author, &String::from_str(&env, &long_content));
}

#[test]
#[should_panic(expected = "tip amount must be positive")]
fn test_tip_rejects_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let tipper = Address::generate(&env);
    let token = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "hello"));

    client.tip(&tipper, &post_id, &token, &0);
}

#[test]
#[should_panic(expected = "verdict must be upheld or dismissed")]
fn test_review_report_rejects_pending_verdict() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _treasury) = setup_contract(&env);

    let user = Address::generate(&env);
    let reporter = Address::generate(&env);
    let token = setup_token(&env, &reporter);
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    let post_id = client.create_post(&user, &String::from_str(&env, "report me"));
    client.grant_role(&admin, &admin, &Role::Moderator);

    client.report_post(
        &reporter,
        &post_id,
        &token,
        &1,
        &BytesN::from_array(&env, &[1u8; 32]),
    );

    let mods = symbol_short!("mods");
    let admins = soroban_sdk::vec![&env, admin.clone()];
    client.create_pool(&admin, &mods, &token, &admins, &1);

    client.review_report(&admin, &admins, &post_id, &reporter, &ReportStatus::Pending);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);
    // Second call must panic
    client.initialize(&admin, &treasury, &0);
}

// A rejected re-initialize must leave the original configuration intact (#690).
// `try_initialize` captures the failure without aborting the test so we can then
// assert the stored treasury/fee were not overwritten by the second call.
#[test]
fn test_initialize_twice_preserves_state() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &treasury, &250);

    // Attempt to re-initialize with different admin/treasury/fee — must fail.
    let other_admin = Address::generate(&env);
    let other_treasury = Address::generate(&env);
    let result = client.try_initialize(&other_admin, &other_treasury, &999);
    assert!(result.is_err());

    // Original treasury and fee remain unchanged.
    assert_eq!(client.get_treasury(), Some(treasury));
    assert_eq!(client.get_fee_bps(), 250);
}

#[test]
fn test_upgrade_by_admin_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);
    let wasm_hash = upload_upgrade_wasm(&env);

    client.upgrade(&admin, &wasm_hash);

    let state: ContractState = env
        .as_contract(&client.address, || {
            env.storage().instance().get(&CONTRACT_STATE)
        })
        .expect("contract state should exist after upgrade");
    assert_eq!(state.version, 2);
    assert_eq!(state.implementation_wasm_hash, Some(wasm_hash));
}

#[test]
#[should_panic]
fn test_upgrade_by_non_admin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let mock_hash = BytesN::from_array(&env, &[1u8; 32]);
    let outsider = Address::generate(&env);

    client.upgrade(&outsider, &mock_hash);
}

#[test]
#[should_panic]
fn test_upgrade_before_initialize_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let mock_hash = BytesN::from_array(&env, &[0u8; 32]);
    let admin = Address::generate(&env);
    client.upgrade(&admin, &mock_hash);
}

#[test]
fn test_upgrade_emits_contract_upgraded_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);
    let wasm_hash = upload_upgrade_wasm(&env);
    let events_before = env.events().all().events().len();
    client.upgrade(&admin, &wasm_hash);

    assert!(env.events().all().events().len() > events_before);
}

#[test]
fn test_admin_can_grant_and_revoke_roles() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);
    let moderator = Address::generate(&env);

    client.grant_role(&admin, &moderator, &Role::Moderator);
    assert!(client.has_role(&moderator, &Role::Moderator));

    client.revoke_role(&admin, &moderator, &Role::Moderator);
    assert!(!client.has_role(&moderator, &Role::Moderator));
}

#[test]
fn test_granted_upgrader_can_upgrade() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);
    let upgrader = Address::generate(&env);
    let wasm_hash = upload_upgrade_wasm(&env);

    client.grant_role(&admin, &upgrader, &Role::Upgrader);
    client.upgrade(&upgrader, &wasm_hash);

    let state: ContractState = env
        .as_contract(&client.address, || {
            env.storage().instance().get(&CONTRACT_STATE)
        })
        .expect("contract state should exist after upgrade");
    assert_eq!(state.implementation_wasm_hash, Some(wasm_hash));
}

// ── Fee boundary tests (issue #196) ─────────────────────────────────────────────

#[test]
fn test_initialize_fee_boundary_max_valid() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initialize with fee_bps = 10_000 (100%) should succeed
    client.initialize(&admin, &treasury, &10_000);
    assert_eq!(client.get_fee_bps(), 10_000);
}

#[test]
#[should_panic(expected = "fee_bps must be between 0 and 10000")]
fn test_initialize_fee_boundary_max_invalid() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initialize with fee_bps = 10_001 (>100%) should panic
    client.initialize(&admin, &treasury, &10_001);
}

#[test]
fn test_set_fee_zero_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    // Set fee to 0 should succeed
    client.set_fee(&admin, &0);
    assert_eq!(client.get_fee_bps(), 0);
}

#[test]
fn test_set_fee_emits_fee_updated_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let _count_before = env.events().all().events().len();
    client.set_fee(&admin, &250);
    let count_after = env.events().all().events().len();

    assert_eq!(client.get_fee_bps(), 250);
    assert!(
        count_after > 0,
        "FeeUpdatedEvent should be emitted: events after set_fee={}",
        count_after
    );
}

#[test]
fn test_set_treasury_emits_treasury_updated_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, old_treasury) = setup_contract(&env);
    let new_treasury = Address::generate(&env);

    let _count_before = env.events().all().events().len();
    client.set_treasury(&admin, &new_treasury);
    let count_after = env.events().all().events().len();

    assert_eq!(client.get_treasury(), Some(new_treasury));
    assert!(
        count_after > 0,
        "TreasuryUpdatedEvent should be emitted: events after set_treasury={}",
        count_after
    );
    assert_ne!(client.get_treasury(), Some(old_treasury));
}

#[test]
#[should_panic]
fn test_set_fee_non_admin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);
    let outsider = Address::generate(&env);

    client.set_fee(&outsider, &100);
}

// ── Username validation tests (issue #195) ───────────────────────────────────────

#[test]
fn test_username_too_short() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // 2-character username is valid (no min length enforcement in contract)
    client.set_profile(&user, &String::from_str(&env, "ab"), &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, String::from_str(&env, "ab"));
}

#[test]
fn test_username_min_length_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // 3-character username should succeed
    client.set_profile(&user, &String::from_str(&env, "abc"), &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, String::from_str(&env, "abc"));
}

#[test]
fn test_username_max_length_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // 32-character username should succeed
    let username_str = "abcdefghijklmnopqrstuvwxyz123456";
    let username = String::from_str(&env, username_str);
    assert_eq!(username.len(), 32);
    client.set_profile(&user, &username, &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, username);
}

#[test]
#[should_panic(expected = "username must be at most 50 characters")]
fn test_username_too_long() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // 51-character username should panic (MAX_NAME_LEN = 50)
    let long_name = "abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGHIJKLMNO";
    assert_eq!(long_name.len(), 51);
    let username = String::from_str(&env, long_name);
    client.set_profile(&user, &username, &token);
}

#[test]
fn test_username_with_space() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // Username with space is valid (no character validation in contract)
    client.set_profile(&user, &String::from_str(&env, "user name"), &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, String::from_str(&env, "user name"));
}

#[test]
fn test_username_with_special_char() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // Username with special character is valid (no character validation in contract)
    client.set_profile(&user, &String::from_str(&env, "user@name"), &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, String::from_str(&env, "user@name"));
}

// ── Unfollow event emission tests (issue #129) ───────────────────────────────────

#[test]
fn test_unfollow_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // First establish a follow relationship
    client.follow(&alice, &bob);

    // Unfollow should emit UnfollowEvent
    client.unfollow(&alice, &bob);

    // Verify at least one event was emitted by unfollow
    let all_events = env.events().all();
    let events = all_events.events();
    assert!(!events.is_empty());
}

#[test]
fn test_unfollow_noop_no_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Unfollow when no relationship exists should not panic
    client.unfollow(&alice, &bob);

    // Verify both indexes are still empty
    assert_eq!(client.get_following(&alice, &0, &10).len(), 0);
    assert_eq!(client.get_followers(&bob, &0, &10).len(), 0);
}

#[test]
fn test_unfollow_nonexistent_relationship_is_noop_emits_event_counts_stay_zero() {
    // unfollow(A, B) when A does not follow B must not panic, must still emit
    // an UnfollowEvent (current behaviour), and must leave both counts at 0.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.unfollow(&alice, &bob);

    // UnfollowEvent is published even though there was no existing edge.
    let all_events = env.events().all();
    let events = all_events.events();
    assert!(!events.is_empty());

    // Counts remain at 0 on both sides.
    assert_eq!(client.get_following(&alice, &0, &10).len(), 0);
    assert_eq!(client.get_followers(&bob, &0, &10).len(), 0);

    let contract_id = client.address.clone();
    env.as_contract(&contract_id, || {
        let following_count: u32 = env
            .storage()
            .persistent()
            .get(&StorageKey::FollowingCount(alice.clone()))
            .unwrap_or(0);
        let followers_count: u32 = env
            .storage()
            .persistent()
            .get(&StorageKey::FollowersCount(bob.clone()))
            .unwrap_or(0);
        assert_eq!(following_count, 0);
        assert_eq!(followers_count, 0);
    });
}

// ── Post content length validation tests (issue #194) ────────────────────────────

#[test]
fn test_post_content_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);

    // Empty content is valid (only max length is enforced)
    let post_id = client.create_post(&author, &String::from_str(&env, ""));
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content, String::from_str(&env, ""));
}

#[test]
fn test_post_content_min_length_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);

    // 1-character content should succeed
    let post_id = client.create_post(&author, &String::from_str(&env, "a"));
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content, String::from_str(&env, "a"));
}

#[test]
fn test_post_content_max_length_valid() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);

    // 280-character content should succeed
    let content_str = "a".repeat(280);
    let content = String::from_str(&env, &content_str);
    assert_eq!(content.len(), 280);
    let post_id = client.create_post(&author, &content);
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content, content);
}

#[test]
#[should_panic(expected = "content must be at most 2000 characters")]
fn test_post_content_too_long() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);

    // 2001-character content should panic (MAX_CONTENT_LEN = 2000)
    let content_str = "a".repeat(2001);
    let content = String::from_str(&env, &content_str);
    assert_eq!(content.len(), 2001);
    client.create_post(&author, &content);
}

// ── get_followers / get_following TTL tests ───────────────────────────────────

#[test]
fn test_get_followers_bumps_followers_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // bob follows alice so alice has a non-empty followers list
    client.follow(&bob, &alice);
    client.get_followers(&alice, &0, &50);

    let contract_id = client.address.clone();

    // StorageKey::FollowersIdx(alice, 0) must have a bumped TTL (new adjacency-set implementation)
    let followers_idx_ttl = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&StorageKey::FollowersIdx(alice.clone(), 0))
    });
    assert!(
        followers_idx_ttl >= LEDGER_THRESHOLD,
        "followers index TTL {followers_idx_ttl} below LEDGER_THRESHOLD"
    );

    // StorageKey::FollowingIdx(alice, 0) must NOT be bumped by get_followers
    let following_idx_exists = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .has(&StorageKey::FollowingIdx(alice.clone(), 0))
    });
    assert!(
        !following_idx_exists,
        "get_followers must not create or bump alice's FollowingIdx key"
    );
}

// ── Issue #346: get_following / get_followers pagination edge cases ───────────

#[test]
fn test_get_following_offset_beyond_list_length_returns_empty() {
    // offset beyond list length must return an empty vec
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.follow(&alice, &bob);

    // alice follows 1 person; offset=100 is way beyond the list
    let page = client.get_following(&alice, &100, &10);
    assert_eq!(
        page.len(),
        0,
        "offset beyond list length must return empty vec"
    );
}

#[test]
fn test_get_following_limit_50_returns_at_most_50() {
    // limit of 50 must return at most 50 results
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    // Follow 60 people
    for _ in 0..60 {
        let followee = Address::generate(&env);
        client.follow(&alice, &followee);
    }

    let page = client.get_following(&alice, &0, &50);
    assert!(page.len() <= 50, "limit=50 must return at most 50 results");
    assert_eq!(page.len(), 50);
}

#[test]
#[should_panic(expected = "limit must be between 1 and 50")]
fn test_get_following_limit_51_panics() {
    // limit of 51 must panic with "limit must be between 1 and 50"
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.follow(&alice, &bob);

    client.get_following(&alice, &0, &51);
}

#[test]
fn test_get_following_mid_list_offset_returns_correct_page() {
    // correct page returned for a mid-list offset
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let mut followees = soroban_sdk::vec![&env];
    for _ in 0..20 {
        let f = Address::generate(&env);
        followees.push_back(f.clone());
        client.follow(&alice, &f);
    }

    // Request page starting at offset 10, limit 5
    let page = client.get_following(&alice, &10, &5);
    assert_eq!(page.len(), 5);
    for i in 0..5u32 {
        assert_eq!(
            page.get(i).unwrap(),
            followees.get(10 + i).unwrap(),
            "mid-list page item {} mismatch",
            i
        );
    }
}

#[test]
fn test_get_followers_offset_beyond_list_length_returns_empty() {
    // offset beyond list length must return an empty vec
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.follow(&bob, &alice); // bob follows alice → alice has 1 follower

    let page = client.get_followers(&alice, &100, &10);
    assert_eq!(
        page.len(),
        0,
        "offset beyond list length must return empty vec"
    );
}

#[test]
fn test_get_followers_limit_50_returns_at_most_50() {
    // limit of 50 must return at most 50 results
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    // 60 people follow alice
    for _ in 0..60 {
        let follower = Address::generate(&env);
        client.follow(&follower, &alice);
    }

    let page = client.get_followers(&alice, &0, &50);
    assert!(page.len() <= 50, "limit=50 must return at most 50 results");
    assert_eq!(page.len(), 50);
}

#[test]
#[should_panic(expected = "limit must be between 1 and 50")]
fn test_get_followers_limit_51_panics() {
    // limit of 51 must panic with "limit must be between 1 and 50"
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    client.follow(&bob, &alice);

    client.get_followers(&alice, &0, &51);
}

#[test]
fn test_get_followers_mid_list_offset_returns_correct_page() {
    // correct page returned for a mid-list offset
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let alice = Address::generate(&env);
    let mut followers = soroban_sdk::vec![&env];
    for _ in 0..20 {
        let f = Address::generate(&env);
        followers.push_back(f.clone());
        client.follow(&f, &alice);
    }

    // Request page starting at offset 10, limit 5
    let page = client.get_followers(&alice, &10, &5);
    assert_eq!(page.len(), 5);
    for i in 0..5u32 {
        assert_eq!(
            page.get(i).unwrap(),
            followers.get(10 + i).unwrap(),
            "mid-list page item {} mismatch",
            i
        );
    }
}

// ── Issue #345: create_post content length fuzz / boundary tests ──────────────

#[test]
fn test_create_post_content_1_char_succeeds() {
    // content of 1 character must succeed
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "x"));
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content, String::from_str(&env, "x"));
    assert_eq!(post.author, author);
}

#[test]
fn test_create_post_content_280_chars_succeeds() {
    // content of exactly 280 characters must succeed
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let content_str = "a".repeat(280);
    let content = String::from_str(&env, &content_str);
    assert_eq!(content.len(), 280);

    let post_id = client.create_post(&author, &content);
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content.len(), 280);
}

#[test]
fn test_create_post_empty_content_panics() {
    // empty content is valid (only max length is enforced in contract)
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, ""));
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content, String::from_str(&env, ""));
}

#[test]
#[should_panic(expected = "content must be at most 2000 characters")]
fn test_create_post_content_281_chars_panics() {
    // content of 2001 characters must panic with a descriptive error
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let content_str = "a".repeat(2001);
    let content = String::from_str(&env, &content_str);
    assert_eq!(content.len(), 2001);
    client.create_post(&author, &content);
}

// ── Pool withdrawal M-of-N integration tests ─────────────────────────────────

/// Helper: create a pool with `n` admins and threshold `m`, deposit `balance`,
/// and return (client, admin, pool_id, token, pool_admins).
fn setup_pool<'a>(
    env: &'a Env,
    n: usize,
    m: u32,
    balance: i128,
) -> (
    LinkoraContractClient<'a>,
    Address,
    soroban_sdk::Symbol,
    Address,
    Vec<Address>,
) {
    let (client, admin, _) = setup_contract(env);

    let mut pool_admins = Vec::new(env);
    let token_owner = Address::generate(env);
    let token = setup_token(env, &token_owner);

    for _ in 0..n {
        pool_admins.push_back(Address::generate(env));
    }

    let depositor = Address::generate(env);
    StellarAssetClient::new(env, &token).mint(&depositor, &(balance + 1000));

    let pool_id = symbol_short!("tpool");
    client.create_pool(&admin, &pool_id, &token, &pool_admins, &m);

    if balance > 0 {
        client.pool_deposit(&depositor, &pool_id, &token, &balance);
    }

    (client, admin, pool_id, token, pool_admins)
}

#[test]
fn test_pool_withdraw_exactly_threshold_2_of_3_succeeds() {
    // 2-of-3: exactly the threshold number of admins (M < N) authorises a withdrawal.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, token, admins) = setup_pool(&env, 3, 2, 300);
    let recipient = Address::generate(&env);

    // Sign with exactly 2 of the 3 admins.
    let signers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &100, &recipient);

    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 200);
    // Recipient must have received the tokens.
    assert_eq!(TokenClient::new(&env, &token).balance(&recipient), 100);
}

#[test]
fn test_pool_withdraw_superset_of_threshold_also_succeeds() {
    // Having more signers than the threshold is always acceptable.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, token, admins) = setup_pool(&env, 5, 3, 500);
    let recipient = Address::generate(&env);

    // All 5 admins sign, threshold is only 3.
    client.pool_withdraw(&admins, &pool_id, &200, &recipient);

    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 300);
    assert_eq!(TokenClient::new(&env, &token).balance(&recipient), 200);
}

#[test]
fn test_pool_withdraw_3_of_5_threshold_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, token, admins) = setup_pool(&env, 5, 3, 600);
    let recipient = Address::generate(&env);

    let signers = vec![
        &env,
        admins.get(0).unwrap(),
        admins.get(2).unwrap(),
        admins.get(4).unwrap(),
    ];
    client.pool_withdraw(&signers, &pool_id, &150, &recipient);

    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 450);
    assert_eq!(TokenClient::new(&env, &token).balance(&recipient), 150);
}

#[test]
fn test_pool_withdraw_sequential_withdrawals_maintain_balance() {
    // Multiple sequential withdrawals reduce the balance correctly.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, token, admins) = setup_pool(&env, 3, 2, 1000);
    let recipient = Address::generate(&env);

    let signers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];

    client.pool_withdraw(&signers, &pool_id, &300, &recipient);
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 700);

    client.pool_withdraw(&signers, &pool_id, &200, &recipient);
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 500);

    client.pool_withdraw(&signers, &pool_id, &500, &recipient);
    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 0);

    assert_eq!(TokenClient::new(&env, &token).balance(&recipient), 1000);
}

#[test]
fn test_pool_withdraw_exact_full_balance_succeeds() {
    // Withdrawing the entire pool balance (boundary case) must succeed.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, token, admins) = setup_pool(&env, 2, 2, 250);
    let recipient = Address::generate(&env);

    let signers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &250, &recipient);

    assert_eq!(client.get_pool(&pool_id).unwrap().balance, 0);
    assert_eq!(TokenClient::new(&env, &token).balance(&recipient), 250);
}

#[test]
fn test_pool_withdraw_event_emitted() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, _, admins) = setup_pool(&env, 2, 2, 400);
    let recipient = Address::generate(&env);

    let signers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &100, &recipient);

    let all_events = env.events().all();
    // At least one event must have been emitted after the withdrawal.
    assert!(
        !all_events.events().is_empty(),
        "withdrawal must emit at least one event"
    );
}

#[test]
fn test_pool_deposit_event_emitted() {
    let env = Env::default();
    env.mock_all_auths();

    // Start with an empty pool so the only deposit is the one under test.
    let (client, _, pool_id, token, _) = setup_pool(&env, 2, 2, 0);
    let depositor = Address::generate(&env);
    StellarAssetClient::new(&env, &token).mint(&depositor, &500);

    let events_before = env.events().all().events().len();
    client.pool_deposit(&depositor, &pool_id, &token, &100);

    // The deposit must add at least one event (the PoolDepositEvent).
    assert!(
        env.events().all().events().len() > events_before,
        "deposit must emit at least one event"
    );
}

#[test]
#[should_panic(expected = "insufficient signers")]
fn test_pool_withdraw_m_of_n_fewer_than_threshold_rejected() {
    // With a 3-of-5 threshold, providing only 2 signers must fail.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, _, admins) = setup_pool(&env, 5, 3, 300);
    let recipient = Address::generate(&env);

    let signers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &100, &recipient);
}

#[test]
#[should_panic(expected = "unauthorized signer")]
fn test_pool_withdraw_m_of_n_non_admin_rejected() {
    // Even if the count meets the threshold, a non-admin signer causes rejection.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, _, admins) = setup_pool(&env, 3, 2, 300);
    let recipient = Address::generate(&env);

    let outsider = Address::generate(&env);
    let signers = vec![&env, admins.get(0).unwrap(), outsider];
    client.pool_withdraw(&signers, &pool_id, &100, &recipient);
}

#[test]
#[should_panic(expected = "low balance")]
fn test_pool_withdraw_m_of_n_exceeds_balance_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, _, admins) = setup_pool(&env, 3, 2, 100);
    let recipient = Address::generate(&env);

    let signers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &101, &recipient);
}

// ── Issue #713: pool_withdraw requires minimum threshold signers ──────────────

#[test]
#[should_panic(expected = "insufficient signers")]
fn test_pool_withdraw_requires_minimum_threshold_signers() {
    // Create a pool with 3 admins and threshold 2.
    // Call pool_withdraw with only 1 signer.
    // Verify it panics with 'insufficient signers'.
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, pool_id, token, admins) = setup_pool(&env, 3, 2, 300);

    let depositor = Address::generate(&env);
    StellarAssetClient::new(&env, &token).mint(&depositor, &500);
    client.pool_deposit(&depositor, &pool_id, &token, &100);

    let recipient = Address::generate(&env);

    // Only 1 signer provided, but threshold is 2 — must panic with "insufficient signers"
    let signers = vec![&env, admins.get(0).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &50, &recipient);
}

// ── Issue #343: full tip flow integration tests ───────────────────────────────

#[test]
fn test_tip_full_flow_no_fee() {
    // fee_bps = 0: entire tip goes to author, treasury receives nothing
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    // Initialize with fee_bps = 0 (no fee)
    client.initialize(&admin, &treasury, &0);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "No-fee tip test"));

    let tip_amount: i128 = 1000;
    client.tip(&tipper, &post_id, &token, &tip_amount);

    // With fee_bps = 0: fee = 0, author gets full amount
    let author_balance = TokenClient::new(&env, &token).balance(&author);
    let treasury_balance = TokenClient::new(&env, &token).balance(&treasury);

    assert_eq!(
        author_balance, tip_amount,
        "author must receive full tip when fee_bps=0"
    );
    assert_eq!(
        treasury_balance, 0,
        "treasury must receive nothing when fee_bps=0"
    );

    // tip_total on the post must be incremented correctly
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(
        post.tip_total, tip_amount,
        "tip_total must equal the gross tip amount"
    );
}

#[test]
fn test_tip_full_flow_with_5_percent_fee() {
    // fee_bps = 500 (5%): verify author and treasury balances and tip_total
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    // Initialize with fee_bps = 500 (5%)
    client.initialize(&admin, &treasury, &500);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "5% fee tip test"));

    let tip_amount: i128 = 1000;
    client.tip(&tipper, &post_id, &token, &tip_amount);

    // fee = 1000 * 500 / 10_000 = 50
    // author gets 1000 - 50 = 950
    let expected_fee: i128 = 50;
    let expected_author: i128 = 950;

    let author_balance = TokenClient::new(&env, &token).balance(&author);
    let treasury_balance = TokenClient::new(&env, &token).balance(&treasury);

    assert_eq!(
        treasury_balance, expected_fee,
        "treasury must receive fee = tip * fee_bps / 10_000"
    );
    assert_eq!(
        author_balance, expected_author,
        "author must receive tip minus fee"
    );

    // tip_total must reflect the net tip amount (after fee deduction)
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(
        post.tip_total, expected_author,
        "tip_total must equal the net author amount after fee deduction"
    );
}

#[test]
fn test_tip_total_increments_across_multiple_tips() {
    // tip_total accumulates correctly across multiple tips
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper1 = Address::generate(&env);
    let tipper2 = Address::generate(&env);

    client.initialize(&admin, &treasury, &500);

    let token = setup_token(&env, &tipper1);
    // Mint tokens for tipper2 as well
    StellarAssetClient::new(&env, &token).mint(&tipper2, &5000);

    let post_id = client.create_post(&author, &String::from_str(&env, "Multi-tip test"));

    // First tip from tipper1 (advance ledger to bypass cooldown)
    client.tip(&tipper1, &post_id, &token, &400);

    // Second tip from tipper2 (different tipper, no cooldown issue)
    client.tip(&tipper2, &post_id, &token, &600);

    let post = client.get_post(&post_id).unwrap();
    // fee_bps=500: tip1=400→fee=20→author=380, tip2=600→fee=30→author=570, total=950
    assert_eq!(
        post.tip_total, 950,
        "tip_total must be the sum of net author amounts after fees"
    );
}

#[test]
fn test_tip_fee_split_matches_fee_bps_config() {
    // fee split must match fee_bps configuration precisely
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    // Use 250 bps (2.5%)
    client.initialize(&admin, &treasury, &250);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "Fee split config test"));

    let tip_amount: i128 = 2000;
    client.tip(&tipper, &post_id, &token, &tip_amount);

    // fee = 2000 * 250 / 10_000 = 50
    // author gets 2000 - 50 = 1950
    let expected_fee: i128 = 50;
    let expected_author: i128 = 1950;

    assert_eq!(
        TokenClient::new(&env, &token).balance(&treasury),
        expected_fee
    );
    assert_eq!(
        TokenClient::new(&env, &token).balance(&author),
        expected_author
    );

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.tip_total, expected_author);
}

#[test]
#[should_panic(expected = "username taken")]
fn test_username_uniqueness_enforced() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let token = Address::generate(&env);

    // User1 registers "alice"
    client.set_profile(&user1, &String::from_str(&env, "alice"), &token);

    // User2 tries to register "alice" - should panic
    client.set_profile(&user2, &String::from_str(&env, "alice"), &token);
}

#[test]
fn test_username_update_by_owner() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    // Register with "alice"
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice")),
        Some(user.clone())
    );

    // Update to "alice_new"
    client.set_profile(&user, &String::from_str(&env, "alice_new"), &token);

    // Old username should be freed
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice")),
        None
    );

    // New username should resolve
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice_new")),
        Some(user)
    );
}

#[test]
fn test_username_freed_on_change() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let token = Address::generate(&env);

    // User1 registers "alice"
    client.set_profile(&user1, &String::from_str(&env, "alice"), &token);

    // User1 changes to "bob"
    client.set_profile(&user1, &String::from_str(&env, "bob"), &token);

    // User2 can now register "alice"
    client.set_profile(&user2, &String::from_str(&env, "alice"), &token);

    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice")),
        Some(user2)
    );
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "bob")),
        Some(user1)
    );
}

#[test]
fn test_pool_admin_added_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool1");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    client.add_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &new_admin,
    );

    // Verify event was emitted
    assert!(
        !env.events().all().events().is_empty(),
        "PoolAdminAddedEvent should be emitted"
    );

    // Verify admin was added
    let pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(pool.admins.len(), 3);
    assert!(pool.admins.iter().any(|a| a == new_admin));
}

#[test]
fn test_pool_admin_removed_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let pool_admin3 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool1");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![
            &env,
            pool_admin1.clone(),
            pool_admin2.clone(),
            pool_admin3.clone(),
        ],
        &2,
    );

    client.remove_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &pool_admin3,
    );

    // Verify event was emitted
    assert!(
        !env.events().all().events().is_empty(),
        "PoolAdminRemovedEvent should be emitted"
    );

    // Verify admin was removed
    let pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(pool.admins.len(), 2);
    assert!(!pool.admins.iter().any(|a| a == pool_admin3));
}

#[test]
fn test_pool_threshold_updated_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool1");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    client.update_pool_threshold(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &1,
    );

    // Verify event was emitted
    assert!(
        !env.events().all().events().is_empty(),
        "PoolThresholdUpdatedEvent should be emitted"
    );

    // Verify threshold was updated
    let pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(pool.threshold, 1);
}

// ── Issue #721: update_pool_threshold rejects threshold of zero ──────────────
//
// Calling update_pool_threshold with threshold 0 must panic with
// "threshold must be positive". The assert! in update_pool_threshold runs
// immediately after bump_instance and before any pool-state reads or writes,
// so the previously stored threshold must remain intact across a rejected
// zero call (Soroban transaction rollback). Two tests are added:
//
//   (A) the literal panic assertion from the issue body, and
//   (B) a state-preservation assertion: after the rejected zero call, the
//       pool's threshold must still equal the value most recently written
//       by a successful call.
//
// The state-preservation test would catch a regression in which someone
// moves the assert! after the storage write, or otherwise causes a partial
// write before the panic (e.g. reordering with a different validation
// rule).

#[test]
#[should_panic(expected = "threshold must be between 1 and 100")]
fn test_update_pool_threshold_zero_panics() {
    // Create a 2-of-2 pool, then call update_pool_threshold with
    // threshold = 0. Verify it panics with "threshold must be positive".
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("p721a");
    // 2-of-2 pool: provides enough valid signers that any panic observed
    // must originate from the threshold-positivity assertion, not from
    // the (later) "insufficient signers" or "unauthorized signer" checks.
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    // The 0-threshold call must panic with "threshold must be positive".
    client.update_pool_threshold(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &0,
    );
}

#[test]
fn test_update_pool_threshold_zero_does_not_mutate_pool_threshold() {
    // Pin down the storage invariant: a rejected threshold = 0 call must
    // not overwrite the previously stored threshold.
    //
    // The function order is:
    //   1. bump_instance   (touches only instance TTL — not pool.threshold)
    //   2. assert!(threshold > 0, ...)  ← panics on threshold = 0
    //   3. read pool from storage
    //   4. signers check
    //   5. write new threshold
    //
    // Because step 2 fires before step 5, the stored threshold must remain
    // at the last successfully written value (here, 1) after step 2 panics.
    // Soroban's transaction-level rollback also guarantees pool.threshold
    // is unchanged, but the post-call read assertion below makes that
    // guarantee explicit and protects against future re-orderings.
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("p721b");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    // Establish a known-good, non-zero threshold of 1 by calling the
    // declared happy path first. After this call, pool.threshold == 1.
    client.update_pool_threshold(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &1,
    );
    assert_eq!(
        client.get_pool(&pool_id).unwrap().threshold,
        1,
        "sanity: happy-path update must succeed"
    );

    // Now call update_pool_threshold with threshold = 0. This MUST panic;
    // try_* exposes the failure as Err so we can inspect pool state after.
    let result = client.try_update_pool_threshold(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &0,
    );
    assert!(
        result.is_err(),
        "update_pool_threshold(.., threshold=0) must return Err"
    );

    // The previously stored threshold must remain intact.
    let pool_after = client.get_pool(&pool_id).unwrap();
    assert_eq!(
        pool_after.threshold, 1,
        "pool.threshold must remain at the last successfully written value (1) \
         after the rejected 0 call, regardless of Soroban's transaction rollback"
    );
}

// ── Issue #124: delete_post success path, unauthorized caller, event emission ─

#[test]
fn test_delete_post_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Hello world"));

    client.delete_post(&author, &post_id);

    assert!(
        client.get_post(&post_id).is_none(),
        "get_post must return None after author deletes their own post"
    );
}

#[test]
#[should_panic(expected = "only author can delete post")]
fn test_delete_post_non_author_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let non_author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Hello world"));

    client.delete_post(&non_author, &post_id);
}

#[test]
#[should_panic(expected = "only author can delete post")]
fn test_delete_post_only_author() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let token = Address::generate(&env);
    client.set_profile(&user_a, &String::from_str(&env, "user_a"), &token);
    client.set_profile(&user_b, &String::from_str(&env, "user_b"), &token);

    let post_id = client.create_post(&user_a, &String::from_str(&env, "Hello from A"));
    client.delete_post(&user_b, &post_id);
}

#[test]
fn test_delete_post_emits_post_deleted_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Event test"));

    client.delete_post(&author, &post_id);

    let all_events = env.events().all();
    let events = all_events.events();
    assert!(
        !events.is_empty(),
        "PostDeleted event must be emitted on successful deletion"
    );
}

#[test]
fn test_delete_post_get_post_count_unaffected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    client.create_post(&author, &String::from_str(&env, "Post 1"));
    let post_id2 = client.create_post(&author, &String::from_str(&env, "Post 2"));

    assert_eq!(client.get_post_count(), 2);
    client.delete_post(&author, &post_id2);
    assert_eq!(
        client.get_post_count(),
        2,
        "get_post_count tracks creation, not existence — deletion must not decrement it"
    );
}

// ── Issue #314: PostDeleted event tests ───────────────────────────────────────

#[test]
fn test_delete_post_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "post to delete"));

    client.delete_post(&author, &post_id);

    let all_events = env.events().all();
    let events = all_events.events();
    assert!(
        !events.is_empty(),
        "PostDeleted event should be emitted on successful deletion"
    );
}

#[test]
#[should_panic(expected = "only author can delete post")]
fn test_delete_post_unauthorized_no_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let non_author = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "test post"));

    // Panics before event emission — PostDeleted is never emitted
    client.delete_post(&non_author, &post_id);
}

// ── Issue #313: TTL extended after profile write ──────────────────────────────

#[test]
fn test_profile_write_extends_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    client.set_profile(&user, &String::from_str(&env, "alice"), &token);

    let contract_id = client.address.clone();

    let profile_ttl = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&StorageKey::Profile(user.clone()))
    });
    assert!(
        profile_ttl >= LEDGER_THRESHOLD,
        "profile TTL {profile_ttl} below LEDGER_THRESHOLD after write"
    );
}

#[test]
fn test_instance_storage_ttl_extended_after_mutation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    // Mutating call should succeed and extend instance storage TTL.
    client.set_fee(&admin, &250);

    // Verify the fee was actually stored correctly (instance storage is working).
    assert_eq!(client.get_fee_bps(), 250);
}

// ── Issue #322: Tip cooldown tests ────────────────────────────────────────────

#[test]
#[should_panic(expected = "tip cooldown not expired")]
fn test_tip_cooldown_rejects_within_window() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);
    // Use a short window so both tips happen within it
    client.set_tip_cooldown_window(&admin, &10);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "cooldown test post"));

    client.tip(&tipper, &post_id, &token, &100);
    // Same ledger → cooldown not expired → panics
    client.tip(&tipper, &post_id, &token, &100);
}

#[test]
fn test_tip_cooldown_allows_after_window() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);
    client.set_tip_cooldown_window(&admin, &10);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "cooldown test post"));

    client.tip(&tipper, &post_id, &token, &100);

    // Advance ledger past the cooldown window
    env.ledger().with_mut(|li| {
        li.sequence_number += 10;
    });

    // Re-tip succeeds after cooldown expires
    client.tip(&tipper, &post_id, &token, &100);

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.tip_total, 200, "tip_total must reflect both tips");
}

// ── Issue #715: set_tip_cooldown_window rejects zero ─────────────────────────

#[test]
#[should_panic(expected = "cooldown_ledgers must be between 1 and")]
fn test_set_tip_cooldown_window_zero_panics() {
    // Calling set_tip_cooldown_window(0) must panic with "cooldown must be positive".
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &treasury, &0);

    // Set a valid window first so we can verify it is unchanged after the panic.
    client.set_tip_cooldown_window(&admin, &5);

    // Passing 0 must panic — the window must stay at 5.
    client.set_tip_cooldown_window(&admin, &0);
}

#[test]
fn test_set_tip_cooldown_window_valid_value_is_stored() {
    // Verify that a valid non-zero value is stored and readable, confirming
    // the original window is never mutated by a rejected zero call (which runs
    // in a separate transaction and panics before any storage write occurs).
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.initialize(&admin, &treasury, &0);

    // Set a valid window and read it back.
    client.set_tip_cooldown_window(&admin, &7);
    assert_eq!(
        client.get_tip_cooldown_window(),
        7,
        "window must equal the last successfully written value"
    );

    // Update to another valid value — confirms the setter works and the
    // storage is never touched by a zero-value call (assert fires first).
    client.set_tip_cooldown_window(&admin, &3);
    assert_eq!(
        client.get_tip_cooldown_window(),
        3,
        "window must reflect the updated valid value"
    );
}

// ── Issue #723: tip amount must be positive (rejects 0 and negatives) ─────

#[test]
#[should_panic(expected = "tip amount must be positive")]
fn test_tip_amount_zero_panics() {
    // Calling tip(tipper, post_id, token, 0) must panic with
    // "tip amount must be positive". The 0 amount is rejected by the very
    // first assertion in tip(), before any auth, post-lookup, cooldown check,
    // token transfer, or tip_total update is performed.
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "tip amount boundary"));

    // The call below MUST panic with the expected message. The
    // #[should_panic(expected = ...)] attribute enforces it.
    client.tip(&tipper, &post_id, &token, &0);
}

#[test]
#[should_panic(expected = "tip amount must be positive")]
fn test_tip_amount_negative_panics() {
    // Calling tip(tipper, post_id, token, -1) must panic with
    // "tip amount must be positive". A negative amount is meaningless and
    // dangerous (it could underflow subtraction of the fee from amount and
    // produce incorrect accounting). The first assertion (`amount > 0`) in
    // tip() rejects it before any state can be touched.
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "tip amount boundary"));

    // The call below MUST panic with the expected message.
    client.tip(&tipper, &post_id, &token, &-1);
}

// ── Issue #321: profile_count decrement on profile deletion ───────────────────

#[test]
fn test_profile_count_decrements_on_delete() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    assert_eq!(
        client.get_profile_count(),
        1,
        "count must be 1 after creation"
    );

    client.delete_profile(&user);
    assert_eq!(
        client.get_profile_count(),
        0,
        "count must decrement to 0 after profile deletion"
    );
    assert!(
        client.get_profile(&user).is_none(),
        "get_profile must return None after deletion"
    );
}

#[test]
fn test_profile_count_never_below_zero() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    client.delete_profile(&user);
    assert_eq!(
        client.get_profile_count(),
        0,
        "count must not go below zero"
    );
}

#[test]
fn test_delete_profile_frees_username() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let token = Address::generate(&env);

    client.set_profile(&user1, &String::from_str(&env, "alice"), &token);
    client.delete_profile(&user1);

    assert!(
        client
            .get_address_by_username(&String::from_str(&env, "alice"))
            .is_none(),
        "username must be freed after profile deletion"
    );

    client.set_profile(&user2, &String::from_str(&env, "alice"), &token);
    assert_eq!(
        client.get_address_by_username(&String::from_str(&env, "alice")),
        Some(user2),
        "freed username must be claimable by another user"
    );
}

#[test]
#[should_panic(expected = "profile does not exist")]
fn test_delete_profile_non_existent_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    client.delete_profile(&user);
}

// ── Issue #132: PROFILE_CREATED_CT semantics ──────────────────────────────────
//
// Design decision: PROFILE_CREATED_CT (stored as "PROF_CT") tracks the total
// number of unique addresses that have ever registered a profile. It is
// incremented exactly once per new address and is never decremented.
// Updating an existing profile does not change the counter.

#[test]
fn test_profile_count_tracks_total_created_never_decrements() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);

    assert_eq!(client.get_profile_count(), 0, "counter starts at zero");

    client.set_profile(&user, &String::from_str(&env, "alice"), &token);
    assert_eq!(
        client.get_profile_count(),
        1,
        "first registration increments counter"
    );

    // Updating the same user's username must NOT increment the counter again.
    client.set_profile(&user, &String::from_str(&env, "alice2"), &token);
    assert_eq!(
        client.get_profile_count(),
        1,
        "profile update must not increment PROFILE_CREATED_CT"
    );

    // A second distinct user adds 1.
    let user2 = Address::generate(&env);
    client.set_profile(&user2, &String::from_str(&env, "bob"), &token);
    assert_eq!(
        client.get_profile_count(),
        2,
        "second registration increments counter"
    );
}

// ── Issue #184: StorageKey typed-key round-trip tests ─────────────────────────

#[test]
fn test_username_index_uses_typed_storage_key() {
    // Verify that the username reverse index is stored and retrieved through
    // StorageKey::UsernameIndex, eliminating the raw (Symbol, String) tuple key.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    let username = String::from_str(&env, "charlie");

    client.set_profile(&user, &username, &token);

    // Read back through the public API (which internally uses StorageKey::UsernameIndex).
    let resolved = client.get_address_by_username(&username);
    assert_eq!(
        resolved,
        Some(user.clone()),
        "username must resolve to owner via typed key"
    );

    // Change username: old index entry must be cleared.
    client.set_profile(&user, &String::from_str(&env, "charlie2"), &token);
    assert!(
        client.get_address_by_username(&username).is_none(),
        "old username must be removed from typed index on update"
    );
}

#[test]
fn test_tip_cooldown_uses_typed_storage_key() {
    // Verify that TipCooldown is enforced via StorageKey::TipCooldown in temp storage.
    // A second tip from the same tipper within the cooldown window must be rejected.
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);
    client.set_tip_cooldown_window(&admin, &100);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "cooldown key test"));

    // First tip succeeds and records the cooldown under StorageKey::TipCooldown.
    client.tip(&tipper, &post_id, &token, &50);

    // Advance ledger past the cooldown window; second tip must succeed.
    env.ledger().with_mut(|li| {
        li.sequence_number += 100;
    });
    client.tip(&tipper, &post_id, &token, &50);

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(
        post.tip_total, 100,
        "both tips must accumulate after cooldown expires"
    );
}

#[test]
fn test_block_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.block_user(&alice, &bob);

    // Verify bob is blocked by alice
    assert!(client.is_blocked(&alice, &bob));
}

#[test]
fn test_unblock_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    // Block first, then unblock
    client.block_user(&alice, &bob);
    client.unblock_user(&alice, &bob);

    // Verify bob is no longer blocked by alice
    assert!(!client.is_blocked(&alice, &bob));
}
// ── DM Key Management Tests ───────────────────────────────────────────────────

#[test]
fn test_publish_and_get_dm_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let dm_key = BytesN::from_array(&env, &[1u8; 32]); // Mock X25519 public key

    // Publish DM key
    client.publish_dm_key(&user, &dm_key);

    // Retrieve DM key
    let retrieved_key = client.get_dm_key(&user);
    assert_eq!(retrieved_key, Some(dm_key));
}

#[test]
fn test_get_dm_key_returns_none_when_not_published() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);

    // User hasn't published a DM key
    let dm_key = client.get_dm_key(&user);
    assert_eq!(dm_key, None);
}

// ── Issue #538: On-chain governance tests ────────────────────────────────────

fn setup_governance(env: &Env) -> (LinkoraContractClient<'_>, Address, Address) {
    let (client, admin, treasury) = setup_contract(env);
    client.gov_init_config(&admin, &60, &100, &200, &50, &30);
    (client, admin, treasury)
}

fn setup_governance_with_pool(
    env: &Env,
) -> (
    LinkoraContractClient<'_>,
    Address,
    Address,
    Symbol,
    Vec<Address>,
) {
    let (client, admin, treasury) = setup_contract(env);
    client.gov_init_config(&admin, &60, &100, &200, &50, &30);

    let pool_admin1 = Address::generate(env);
    let pool_admin2 = Address::generate(env);
    let token = setup_token(env, &pool_admin1);
    let pool_id = symbol_short!("vetpool");
    let pool_admins = vec![env, pool_admin1.clone(), pool_admin2.clone()];
    client.create_pool(&admin, &pool_id, &token, &pool_admins, &2);

    (client, admin, treasury, pool_id, pool_admins)
}

#[test]
fn test_gov_happy_path_propose_vote_execute() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    let voter1 = Address::generate(&env);
    let voter2 = Address::generate(&env);
    let voter3 = Address::generate(&env);
    client.gov_vote(&voter1, &proposal_id, &true);
    client.gov_vote(&voter2, &proposal_id, &true);
    client.gov_vote(&voter3, &proposal_id, &false);

    let proposal = client.gov_get_proposal(&proposal_id);
    assert_eq!(proposal.votes_for, 2);
    assert_eq!(proposal.votes_against, 1);

    env.ledger().with_mut(|li| {
        li.sequence_number += 200 + 100;
    });

    client.gov_execute(&proposal_id);

    assert_eq!(client.get_fee_bps(), 500);
    let executed = client.gov_get_proposal(&proposal_id);
    assert_eq!(executed.status, GovStatus::Executed);
}

#[test]
fn test_gov_quorum_decay_effective_quorum_decreases_over_time() {
    // Verify that effective_quorum decreases as ledgers elapse.
    let env = Env::default();
    env.mock_all_auths();
    // quorum=60, time_lock=50, vote_window=100, decay_rate=500 bps (5%/ledger), floor=10
    let (client, admin, _) = setup_contract(&env);
    client.gov_init_config(&admin, &60, &50, &100, &500, &10);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &100, &None);

    // At creation time quorum equals the configured value.
    let q0 = client.effective_quorum(&proposal_id);
    assert_eq!(q0, 60, "quorum at creation must equal configured value");

    // Advance 100 ledgers: decay = 100 * 500 / 10000 = 5 → effective = max(10, 60-5) = 55
    env.ledger().with_mut(|li| {
        li.sequence_number += 100;
    });
    let q100 = client.effective_quorum(&proposal_id);
    assert_eq!(q100, 55, "quorum should decay to 55 after 100 ledgers");
    assert!(q100 < q0, "quorum must decrease with elapsed ledgers");

    // Advance another 900 ledgers (total 1000): decay = 1000*500/10000 = 50 → max(10, 60-50)=10
    env.ledger().with_mut(|li| {
        li.sequence_number += 900;
    });
    let q1000 = client.effective_quorum(&proposal_id);
    assert_eq!(
        q1000, 10,
        "quorum should decay to floor (10) after 1000 ledgers"
    );
    assert!(q1000 < q100, "quorum must continue decreasing");
}

#[test]
#[should_panic(expected = "quorum not met")]
fn test_gov_quorum_decay_proposal_fails_below_floor() {
    // Even with maximum quorum decay (floor=30), a proposal with 20% approval fails.
    let env = Env::default();
    env.mock_all_auths();
    // setup_governance uses: quorum=60, time_lock=100, vote_window=200, decay=50, floor=30
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &100, &None);

    // 1 for, 4 against = 20% approval — below the 30% floor
    let v1 = Address::generate(&env);
    let v2 = Address::generate(&env);
    let v3 = Address::generate(&env);
    let v4 = Address::generate(&env);
    let v5 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);
    client.gov_vote(&v2, &proposal_id, &false);
    client.gov_vote(&v3, &proposal_id, &false);
    client.gov_vote(&v4, &proposal_id, &false);
    client.gov_vote(&v5, &proposal_id, &false);

    // Advance far enough for maximum decay — floor stays at 30
    env.ledger().with_mut(|li| {
        li.sequence_number += 10_000; // well past vote_window + time_lock
    });

    // effective_quorum decays to floor=30; 20% < 30% → must panic "quorum not met"
    client.gov_execute(&proposal_id);
}

#[test]
#[should_panic(expected = "quorum not met")]
fn test_gov_quorum_not_met_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &100, &None);

    // 1 for, 4 against = 20% approval
    let v1 = Address::generate(&env);
    let v2 = Address::generate(&env);
    let v3 = Address::generate(&env);
    let v4 = Address::generate(&env);
    let v5 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);
    client.gov_vote(&v2, &proposal_id, &false);
    client.gov_vote(&v3, &proposal_id, &false);
    client.gov_vote(&v4, &proposal_id, &false);
    client.gov_vote(&v5, &proposal_id, &false);

    env.ledger().with_mut(|li| {
        li.sequence_number += 200 + 100;
    });

    // 20% < 30% (floor) → quorum not met
    client.gov_execute(&proposal_id);
}

#[test]
fn test_gov_quorum_decay_allows_passage() {
    let env = Env::default();
    env.mock_all_auths();

    // Config: quorum=60, decay_rate=50 bps/ledger, floor=30
    // After 200 ledgers (vote window), execution at 300+ ledgers
    // Decay at ledger 300: elapsed from created = 300, decay = 300*50/10000 = 1
    // effective_quorum = max(30, 60-1) = 59
    // Need higher decay for meaningful test. Let's use custom config.
    let (client, admin, _) = setup_contract(&env);
    // quorum=60, time_lock=50, vote_window=100, decay_rate=1000 bps (10%/ledger), floor=30
    client.gov_init_config(&admin, &60, &50, &100, &1000, &30);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &200, &None);

    // Vote: 7 for, 13 against = 35% approval (below 60% quorum, but above 30% floor)
    for _ in 0..7 {
        let v = Address::generate(&env);
        client.gov_vote(&v, &proposal_id, &true);
    }
    for _ in 0..13 {
        let v = Address::generate(&env);
        client.gov_vote(&v, &proposal_id, &false);
    }

    // Advance past vote window + time_lock: need elapsed >= 150
    // At elapsed=300: decay = 300 * 1000 / 10000 = 30 → effective_quorum = max(30, 60-30) = 30
    env.ledger().with_mut(|li| {
        li.sequence_number += 300;
    });

    let eff_q = client.effective_quorum(&proposal_id);
    assert_eq!(eff_q, 30, "effective quorum should decay to floor");

    // 35% >= 30% → passes with decay
    client.gov_execute(&proposal_id);
    assert_eq!(client.get_fee_bps(), 200);
}

#[test]
fn test_gov_veto_during_timelock() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _, pool_id, pool_admins) = setup_governance_with_pool(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    let v1 = Address::generate(&env);
    let v2 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);
    client.gov_vote(&v2, &proposal_id, &true);

    // Advance past vote window (200) but within time-lock (200 + 100)
    env.ledger().with_mut(|li| {
        li.sequence_number += 250;
    });

    client.gov_veto(&pool_admins, &pool_id, &proposal_id);

    let proposal = client.gov_get_proposal(&proposal_id);
    assert_eq!(proposal.status, GovStatus::Vetoed);
}

#[test]
#[should_panic(expected = "proposal not active")]
fn test_gov_veto_prevents_execution() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _, pool_id, pool_admins) = setup_governance_with_pool(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    let v1 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);

    // Advance past vote window but within time-lock
    env.ledger().with_mut(|li| {
        li.sequence_number += 250;
    });

    client.gov_veto(&pool_admins, &pool_id, &proposal_id);

    // Advance past time-lock
    env.ledger().with_mut(|li| {
        li.sequence_number += 100;
    });

    // Should panic because proposal is vetoed (status != Active)
    client.gov_execute(&proposal_id);
}

#[test]
#[should_panic(expected = "already voted")]
fn test_gov_double_vote_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    let voter = Address::generate(&env);
    client.gov_vote(&voter, &proposal_id, &true);
    client.gov_vote(&voter, &proposal_id, &false);
}

#[test]
fn test_gov_emergency_bypass_set_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_governance(&env);

    let old_fee = client.get_fee_bps();
    client.set_fee(&admin, &999);
    assert_eq!(client.get_fee_bps(), 999);
    assert_ne!(old_fee, 999, "fee must have changed via emergency bypass");
}

#[test]
fn test_gov_emergency_bypass_set_treasury() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _treasury) = setup_governance(&env);

    let new_treasury = Address::generate(&env);
    client.set_treasury(&admin, &new_treasury);
    assert_eq!(
        client.get_treasury(),
        Some(new_treasury),
        "treasury must be updated via emergency bypass"
    );
}

#[test]
fn test_publish_dm_key_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let dm_key = BytesN::from_array(&env, &[2u8; 32]);

    let _count_before = env.events().all().events().len();
    client.publish_dm_key(&user, &dm_key);
    let count_after = env.events().all().events().len();

    let dm_key_check = client.get_dm_key(&user);
    assert_eq!(dm_key_check, Some(dm_key));

    assert!(
        count_after > 0,
        "DmKeyPublishedEvent should be emitted: events after publish_dm_key={}",
        count_after
    );
}

#[test]
fn test_publish_dm_key_update_overwrites_previous() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let old_key = BytesN::from_array(&env, &[3u8; 32]);
    let new_key = BytesN::from_array(&env, &[4u8; 32]);

    // Publish first key
    client.publish_dm_key(&user, &old_key);
    assert_eq!(client.get_dm_key(&user), Some(old_key));

    // Update with new key
    client.publish_dm_key(&user, &new_key);
    assert_eq!(client.get_dm_key(&user), Some(new_key));
}

#[test]
fn test_dm_key_storage_uses_typed_key() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let dm_key = BytesN::from_array(&env, &[5u8; 32]);

    client.publish_dm_key(&user, &dm_key);

    // Verify TTL is extended (indicating proper typed key usage)
    let contract_id = client.address.clone();
    let dm_key_ttl = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&StorageKey::DmPublicKey(user.clone()))
    });
    assert!(
        dm_key_ttl >= LEDGER_THRESHOLD,
        "DM key TTL should be extended after write"
    );
}

#[test]
fn test_migrate_follow_graph_chunk_safe() {
    // Migration can be split across multiple calls.
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let charlie = Address::generate(&env);

    let contract_id = client.address.clone();

    // Write old-style entries for both users
    env.as_contract(&contract_id, || {
        let following_key_a = StorageKey::Following(alice.clone());
        let list_a = vec![&env, charlie.clone()];
        env.storage().persistent().set(&following_key_a, &list_a);
        env.storage()
            .persistent()
            .extend_ttl(&following_key_a, LEDGER_THRESHOLD, LEDGER_BUMP);

        let following_key_b = StorageKey::Following(bob.clone());
        let list_b = vec![&env, charlie.clone()];
        env.storage().persistent().set(&following_key_b, &list_b);
        env.storage()
            .persistent()
            .extend_ttl(&following_key_b, LEDGER_THRESHOLD, LEDGER_BUMP);
    });

    // Migrate in two separate chunks
    client.migrate_follow_graph(&admin, &vec![&env, alice.clone()]);
    client.migrate_follow_graph(&admin, &vec![&env, bob.clone()]);

    // Both should have their following list migrated
    let alice_following = client.get_following(&alice, &0, &50);
    assert_eq!(alice_following.len(), 1);

    let bob_following = client.get_following(&bob, &0, &50);
    assert_eq!(bob_following.len(), 1);

    // Charlie should have 2 followers from the migration
    let charlie_followers = client.get_followers(&charlie, &0, &50);
    assert_eq!(charlie_followers.len(), 2);
}

#[test]
#[should_panic(expected = "batch size must not exceed 50 users")]
fn test_migrate_follow_graph_rejects_oversized_batch() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let mut users = Vec::new(&env);
    for _ in 0..51 {
        users.push_back(Address::generate(&env));
    }

    client.migrate_follow_graph(&admin, &users);
}

#[test]
fn test_duplicate_follow_is_idempotent() {
    // Calling follow(A, B) twice must not increment FollowersCount/FollowingCount
    // beyond 1, and must not create a second index entry for the same edge.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.follow(&alice, &bob);
    client.follow(&alice, &bob);

    assert_eq!(client.get_following(&alice, &0, &50).len(), 1);
    assert_eq!(client.get_followers(&bob, &0, &50).len(), 1);

    let contract_id = client.address.clone();
    env.as_contract(&contract_id, || {
        let following_count: u32 = env
            .storage()
            .persistent()
            .get(&StorageKey::FollowingCount(alice.clone()))
            .unwrap_or(0);
        let followers_count: u32 = env
            .storage()
            .persistent()
            .get(&StorageKey::FollowersCount(bob.clone()))
            .unwrap_or(0);
        assert_eq!(following_count, 1);
        assert_eq!(followers_count, 1);

        // No second index entry should have been written for the duplicate follow.
        assert!(!env
            .storage()
            .persistent()
            .has(&StorageKey::FollowingIdx(alice.clone(), 1)));
        assert!(!env
            .storage()
            .persistent()
            .has(&StorageKey::FollowersIdx(bob.clone(), 1)));
    });
}

#[test]
fn test_follow_unfollow_refollow_consistency() {
    // Follow, unfollow, then re-follow should result in exactly 1 entry.
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.follow(&alice, &bob);
    assert_eq!(client.get_following(&alice, &0, &50).len(), 1);
    assert_eq!(client.get_followers(&bob, &0, &50).len(), 1);

    client.unfollow(&alice, &bob);
    assert_eq!(client.get_following(&alice, &0, &50).len(), 0);
    assert_eq!(client.get_followers(&bob, &0, &50).len(), 0);

    client.follow(&alice, &bob);
    assert_eq!(client.get_following(&alice, &0, &50).len(), 1);
    assert_eq!(client.get_followers(&bob, &0, &50).len(), 1);
}

#[test]
#[should_panic(expected = "time-lock not expired")]
fn test_gov_execute_before_timelock_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    let v1 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);

    // Only advance past vote window, not time-lock
    env.ledger().with_mut(|li| {
        li.sequence_number += 210;
    });

    client.gov_execute(&proposal_id);
}

#[test]
#[should_panic(expected = "vote window closed")]
fn test_gov_vote_after_window_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    env.ledger().with_mut(|li| {
        li.sequence_number += 201;
    });

    let voter = Address::generate(&env);
    client.gov_vote(&voter, &proposal_id, &true);
}

#[test]
fn test_gov_tip_cooldown_parameter_change() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::TipCooldownWindow, &5000, &None);

    let v1 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);

    env.ledger().with_mut(|li| {
        li.sequence_number += 300;
    });

    client.gov_execute(&proposal_id);
    assert_eq!(client.get_tip_cooldown_window(), 5000);
}

#[test]
fn test_gov_config_init_and_read() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let config = client.gov_get_config();
    assert_eq!(config.quorum, 60);
    assert_eq!(config.time_lock_ledgers, 100);
    assert_eq!(config.vote_window_ledgers, 200);
    assert_eq!(config.quorum_decay_rate_bps, 50);
    assert_eq!(config.quorum_floor, 30);
}

#[test]
fn test_gov_proposal_get() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &750, &None);

    let proposal = client.gov_get_proposal(&proposal_id);
    assert_eq!(proposal.id, proposal_id);
    assert_eq!(proposal.proposer, proposer);
    assert_eq!(proposal.parameter, GovParameter::FeeBps);
    assert_eq!(proposal.new_value, 750);
    assert_eq!(proposal.votes_for, 0);
    assert_eq!(proposal.votes_against, 0);
    assert_eq!(proposal.status, GovStatus::Active);
}

#[test]
#[should_panic(expected = "veto only during time-lock window")]
fn test_gov_veto_before_vote_window_ends_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _, pool_id, pool_admins) = setup_governance_with_pool(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    // Still within vote window
    env.ledger().with_mut(|li| {
        li.sequence_number += 100;
    });

    client.gov_veto(&pool_admins, &pool_id, &proposal_id);
}

#[test]
#[should_panic(expected = "veto only during time-lock window")]
fn test_gov_veto_after_timelock_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _, pool_id, pool_admins) = setup_governance_with_pool(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    // Past time-lock window entirely
    env.ledger().with_mut(|li| {
        li.sequence_number += 400;
    });

    client.gov_veto(&pool_admins, &pool_id, &proposal_id);
}

#[test]
fn test_gov_effective_quorum_at_creation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    let eff_q = client.effective_quorum(&proposal_id);
    assert_eq!(
        eff_q, 60,
        "effective quorum should equal base quorum at creation"
    );
}

#[test]
fn test_gov_change_gov_quorum_via_governance() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::GovQuorum, &50, &None);

    let v1 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);

    env.ledger().with_mut(|li| {
        li.sequence_number += 300;
    });

    client.gov_execute(&proposal_id);

    let config = client.gov_get_config();
    assert_eq!(config.quorum, 50);
}

#[test]
fn test_gov_multiple_proposals() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let proposer = Address::generate(&env);
    let id1 = client.gov_propose(&proposer, &GovParameter::FeeBps, &100, &None);
    let id2 = client.gov_propose(&proposer, &GovParameter::FeeBps, &200, &None);

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);

    let p1 = client.gov_get_proposal(&id1);
    let p2 = client.gov_get_proposal(&id2);
    assert_eq!(p1.new_value, 100);
    assert_eq!(p2.new_value, 200);
}

#[test]
fn test_gov_treasury_change_via_governance() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _) = setup_governance(&env);

    let new_treasury = Address::generate(&env);
    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(
        &proposer,
        &GovParameter::Treasury,
        &0,
        &Some(new_treasury.clone()),
    );

    let v1 = Address::generate(&env);
    client.gov_vote(&v1, &proposal_id, &true);

    env.ledger().with_mut(|li| {
        li.sequence_number += 300;
    });

    client.gov_execute(&proposal_id);
    assert_eq!(client.get_treasury(), Some(new_treasury));
}

#[test]
#[should_panic(expected = "insufficient signers")]
fn test_gov_veto_insufficient_pool_signers_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, _, pool_id, pool_admins) = setup_governance_with_pool(&env);

    let proposer = Address::generate(&env);
    let proposal_id = client.gov_propose(&proposer, &GovParameter::FeeBps, &500, &None);

    env.ledger().with_mut(|li| {
        li.sequence_number += 250;
    });

    // Only 1 signer when pool threshold is 2
    let single_signer = vec![&env, pool_admins.get(0).unwrap()];
    client.gov_veto(&single_signer, &pool_id, &proposal_id);
}

// ── delete_post removes ID from author index and get_post returns None ─────────

#[test]
fn test_delete_post_removed_from_author_index_and_get_post_none() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let id1 = client.create_post(&author, &String::from_str(&env, "post 1"));
    let id2 = client.create_post(&author, &String::from_str(&env, "post 2"));
    let id3 = client.create_post(&author, &String::from_str(&env, "post 3"));

    // Delete the middle post
    client.delete_post(&author, &id2);

    // get_posts_by_author must no longer include the deleted post ID
    let page = client.get_posts_by_author(&author, &0, &10);
    assert_eq!(
        page.len(),
        2,
        "deleted post must be removed from author index"
    );
    assert!(
        !page.iter().any(|id| id == id2),
        "deleted post ID must not appear in get_posts_by_author"
    );
    assert!(page.iter().any(|id| id == id1));
    assert!(page.iter().any(|id| id == id3));

    // get_post must return None for the deleted post
    assert!(
        client.get_post(&id2).is_none(),
        "get_post must return None for a deleted post"
    );
    // The surviving posts are still retrievable
    assert!(client.get_post(&id1).is_some());
    assert!(client.get_post(&id3).is_some());
}

// ── create_post content boundary: 280 succeeds, 281 panics ────────────────────

#[test]
fn test_create_post_280_chars_boundary_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let content_str = "a".repeat(280);
    let content = String::from_str(&env, &content_str);
    assert_eq!(content.len(), 280);

    // Exactly 280 characters must succeed
    let post_id = client.create_post(&author, &content);
    let post = client.get_post(&post_id).unwrap();
    assert_eq!(post.content, content);
    assert_eq!(post.content.len(), 280);
}

#[test]
#[should_panic(expected = "content must be at most 2000 characters")]
fn test_create_post_281_chars_boundary_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let content_str = "a".repeat(2001);
    let content = String::from_str(&env, &content_str);
    assert_eq!(content.len(), 2001);

    // 2001 characters must panic with "content must be at most 2000 characters"
    client.create_post(&author, &content);
}

// ── Tip cooldown of 100 ledgers: reject immediate re-tip, allow after window ───

#[test]
#[should_panic(expected = "tip cooldown not expired")]
fn test_tip_cooldown_100_ledgers_immediate_retip_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);
    client.set_tip_cooldown_window(&admin, &100);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "cooldown 100 post"));

    // First tip succeeds
    client.tip(&tipper, &post_id, &token, &100);
    // Immediate second tip within the 100-ledger window must panic
    client.tip(&tipper, &post_id, &token, &100);
}

#[test]
fn test_tip_cooldown_100_ledgers_allows_after_advance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let author = Address::generate(&env);
    let tipper = Address::generate(&env);

    client.initialize(&admin, &treasury, &0);
    client.set_tip_cooldown_window(&admin, &100);

    let token = setup_token(&env, &tipper);
    let post_id = client.create_post(&author, &String::from_str(&env, "cooldown 100 post"));

    // First tip succeeds
    client.tip(&tipper, &post_id, &token, &100);

    // Advance the ledger by exactly the 100-ledger cooldown window
    env.ledger().with_mut(|li| {
        li.sequence_number += 100;
    });

    // Tip succeeds again once the cooldown has elapsed
    client.tip(&tipper, &post_id, &token, &100);

    let post = client.get_post(&post_id).unwrap();
    assert_eq!(
        post.tip_total, 200,
        "both tips must accumulate once the 100-ledger cooldown expires"
    );
}

// ── pool_withdraw on a zero-balance pool panics with "low balance" ────────────

#[test]
#[should_panic(expected = "low balance")]
fn test_pool_withdraw_zero_balance_panics_low_balance() {
    let env = Env::default();
    env.mock_all_auths();

    // 1-of-1 pool with zero balance (no deposit performed)
    let (client, _, pool_id, _token, admins) = setup_pool(&env, 1, 1, 0);
    let recipient = Address::generate(&env);

    assert_eq!(
        client.get_pool(&pool_id).unwrap().balance,
        0,
        "pool must start with a zero balance"
    );

    // Withdrawing any positive amount from an empty pool must panic with "low balance"
    let signers = vec![&env, admins.get(0).unwrap()];
    client.pool_withdraw(&signers, &pool_id, &1, &recipient);
}

// ── Issue #678: add_pool_admin duplicate rejection ────────────────────────────

#[test]
#[should_panic(expected = "admin already exists")]
fn test_add_pool_admin_duplicate_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool1");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    let initial_pool = client.get_pool(&pool_id).unwrap();
    let initial_admin_count = initial_pool.admins.len();
    assert_eq!(initial_admin_count, 2);

    client.add_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &pool_admin1,
    );
}

#[test]
fn test_add_pool_admin_duplicate_preserves_admin_list_length() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool2");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    let before_pool = client.get_pool(&pool_id).unwrap();
    let before_count = before_pool.admins.len();

    let result = client.try_add_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &pool_admin1,
    );
    assert!(result.is_err());

    let after_pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(after_pool.admins.len(), before_count);
}

// ── Issue #679: like_post idempotency - duplicate like is ignored ─────────────

#[test]
fn test_like_post_idempotency_duplicate_ignored() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let user = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "Idempotency test"));

    client.like_post(&user, &post_id);
    assert_eq!(client.get_like_count(&post_id), 1);
    assert!(client.has_liked(&user, &post_id));

    client.like_post(&user, &post_id);
    assert_eq!(client.get_like_count(&post_id), 1);
    assert!(client.has_liked(&user, &post_id));
}

#[test]
fn test_like_post_second_call_is_no_op() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let author = Address::generate(&env);
    let user = Address::generate(&env);
    let post_id = client.create_post(&author, &String::from_str(&env, "No-op test"));

    client.like_post(&user, &post_id);
    let like_count_after_first = client.get_like_count(&post_id);
    let has_liked_after_first = client.has_liked(&user, &post_id);

    client.like_post(&user, &post_id);
    let like_count_after_second = client.get_like_count(&post_id);
    let has_liked_after_second = client.has_liked(&user, &post_id);

    assert_eq!(like_count_after_second, like_count_after_first);
    assert_eq!(has_liked_after_second, has_liked_after_first);
    assert_eq!(like_count_after_second, 1);
}

// ── Issue #680: remove_pool_admin threshold unreachable validation ────────────

#[test]
#[should_panic(expected = "threshold unreachable after removal")]
fn test_remove_pool_admin_threshold_unreachable() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool3");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    let pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(pool.admins.len(), 2);
    assert_eq!(pool.threshold, 2);

    client.remove_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &pool_admin1,
    );
}

#[test]
fn test_remove_pool_admin_threshold_valid_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let pool_admin3 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("pool4");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![
            &env,
            pool_admin1.clone(),
            pool_admin2.clone(),
            pool_admin3.clone(),
        ],
        &2,
    );

    let before_pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(before_pool.admins.len(), 3);

    client.remove_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &pool_admin3,
    );

    let after_pool = client.get_pool(&pool_id).unwrap();
    assert_eq!(after_pool.admins.len(), 2);
    assert_eq!(after_pool.threshold, 2);
    assert!(!after_pool.admins.iter().any(|a| a == pool_admin3));
}

// ── Issue #685: get_following/get_followers empty vec when offset beyond list ─

#[test]
fn test_get_following_offset_beyond_list_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let user_c = Address::generate(&env);
    let user_d = Address::generate(&env);

    client.follow(&user_a, &user_b);
    client.follow(&user_a, &user_c);
    client.follow(&user_a, &user_d);

    let following = client.get_following(&user_a, &0, &10);
    assert_eq!(following.len(), 3);

    let empty_result = client.get_following(&user_a, &10, &10);
    assert_eq!(empty_result.len(), 0);
}

#[test]
fn test_get_followers_offset_beyond_list_returns_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let user_c = Address::generate(&env);
    let user_d = Address::generate(&env);

    client.follow(&user_b, &user_a);
    client.follow(&user_c, &user_a);
    client.follow(&user_d, &user_a);

    let followers = client.get_followers(&user_a, &0, &10);
    assert_eq!(followers.len(), 3);

    let empty_result = client.get_followers(&user_a, &10, &10);
    assert_eq!(empty_result.len(), 0);
}

#[test]
fn test_get_following_large_offset_no_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);
    let user_c = Address::generate(&env);

    client.follow(&user_a, &user_b);
    client.follow(&user_a, &user_c);

    let result = client.get_following(&user_a, &100, &10);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_get_followers_large_offset_no_panic() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LinkoraContract, ());
    let client = LinkoraContractClient::new(&env, &contract_id);

    let user_a = Address::generate(&env);
    let user_b = Address::generate(&env);

    client.follow(&user_b, &user_a);

    let result = client.get_followers(&user_a, &100, &10);
    assert_eq!(result.len(), 0);
}

// ── Issue #717: verify username of 33 characters is rejected ─────────────────

#[test]
#[should_panic(expected = "username must be at most 50 characters")]
fn test_717_set_profile_username_33_chars_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    // 51-character username must panic (MAX_NAME_LEN = 50)
    let long_name = "abcdefghijklmnopqrstuvwxyz1234567890ABCDEFGHIJKLMNO";
    assert_eq!(long_name.len(), 51);
    let username = String::from_str(&env, long_name);
    client.set_profile(&user, &username, &token);
}

#[test]
fn test_717_set_profile_username_32_chars_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let user = Address::generate(&env);
    let token = Address::generate(&env);
    // exactly 32-character username must succeed
    let username_str = "abcdefghijklmnopqrstuvwxyz123456";
    let username = String::from_str(&env, username_str);
    assert_eq!(username.len(), 32);
    client.set_profile(&user, &username, &token);
    let profile = client.get_profile(&user).unwrap();
    assert_eq!(profile.username, username);
}

// ── Issue #1393: test coverage for get_pool_admins ────────────────────────────

#[test]
fn test_get_pool_admins_returns_admin_list() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let pool_admin3 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("testpool");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![
            &env,
            pool_admin1.clone(),
            pool_admin2.clone(),
            pool_admin3.clone(),
        ],
        &2,
    );

    let admins = client.get_pool_admins(&pool_id);
    assert_eq!(admins.len(), 3);
    assert!(admins.iter().any(|a| a == pool_admin1));
    assert!(admins.iter().any(|a| a == pool_admin2));
    assert!(admins.iter().any(|a| a == pool_admin3));
}

#[test]
#[should_panic(expected = "pool not found")]
fn test_get_pool_admins_nonexistent_pool_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = setup_contract(&env);

    let pool_id = symbol_short!("missing");
    client.get_pool_admins(&pool_id);
}

#[test]
fn test_get_pool_admins_after_admin_added() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("testpool");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &2,
    );

    client.add_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &new_admin,
    );

    let admins = client.get_pool_admins(&pool_id);
    assert_eq!(admins.len(), 3);
    assert!(admins.iter().any(|a| a == new_admin));
}

#[test]
fn test_get_pool_admins_after_admin_removed() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, _) = setup_contract(&env);

    let pool_admin1 = Address::generate(&env);
    let pool_admin2 = Address::generate(&env);
    let pool_admin3 = Address::generate(&env);
    let token = setup_token(&env, &pool_admin1);

    let pool_id = symbol_short!("testpool");
    client.create_pool(
        &admin,
        &pool_id,
        &token,
        &vec![
            &env,
            pool_admin1.clone(),
            pool_admin2.clone(),
            pool_admin3.clone(),
        ],
        &2,
    );

    client.remove_pool_admin(
        &vec![&env, pool_admin1.clone(), pool_admin2.clone()],
        &pool_id,
        &pool_admin3,
    );

    let admins = client.get_pool_admins(&pool_id);
    assert_eq!(admins.len(), 2);
    assert!(!admins.iter().any(|a| a == pool_admin3));
}
