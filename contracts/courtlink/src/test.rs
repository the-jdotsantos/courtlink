#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::Address as _,
        token, Address, BytesN, Env, String,
    };

    use crate::{BookingStatus, CourtLink, CourtLinkClient, Error};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_id(env: &Env, seed: u8) -> BytesN<32> {
        BytesN::from_array(env, &[seed; 32])
    }

    fn setup(
        env: &Env,
    ) -> (
        CourtLinkClient,
        Address, // admin
        Address, // token
        token::Client,
        token::StellarAssetClient,
    ) {
        let admin = Address::generate(env);
        let token_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token_addr = token_contract.address();
        let token_client = token::Client::new(env, &token_addr);
        let token_admin = token::StellarAssetClient::new(env, &token_addr);

        let contract_id = env.register(CourtLink, ());
        let client = CourtLinkClient::new(env, &contract_id);
        client.initialize(&admin, &token_addr);

        (client, admin, token_addr, token_client, token_admin)
    }

    // -----------------------------------------------------------------------
    // Test 1 — Happy path
    // Player books a slot, funds move to escrow; court owner completes; gets paid.
    // -----------------------------------------------------------------------
    #[test]
    fn test_book_and_complete_happy_path() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, admin, _token_addr, token_client, token_admin) = setup(&env);

        let player = Address::generate(&env);
        let court_owner = Address::generate(&env);
        let amount: i128 = 50_000_000; // 5 XLM in stroops

        // Fund player wallet
        token_admin.mint(&player, &amount);

        let booking_id = make_id(&env, 0x01);
        let court_id = String::from_str(&env, "Cubao Court 1");
        let slot_ts: u64 = 1_700_000_000; // arbitrary future unix ts

        // -- Book the slot --
        client.book_slot(
            &booking_id,
            &player,
            &court_owner,
            &court_id,
            &slot_ts,
            &amount,
        );

        // Player's balance should be zero (all in escrow)
        assert_eq!(token_client.balance(&player), 0);

        // -- Court owner completes the booking --
        let owner_balance_before = token_client.balance(&court_owner);
        client.complete_booking(&booking_id, &court_owner);
        let owner_balance_after = token_client.balance(&court_owner);

        assert_eq!(
            owner_balance_after - owner_balance_before,
            amount,
            "Court owner must receive the full escrowed amount"
        );

        // Booking status must be Completed
        let record = client.get_booking(&booking_id);
        assert_eq!(record.status, BookingStatus::Completed);
    }

    // -----------------------------------------------------------------------
    // Test 2 — Edge case: duplicate booking_id is rejected
    // -----------------------------------------------------------------------
    #[test]
    fn test_duplicate_booking_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _admin, _token, token_client, token_admin) = setup(&env);

        let player = Address::generate(&env);
        let court_owner = Address::generate(&env);
        let amount: i128 = 50_000_000;

        token_admin.mint(&player, &(amount * 2));

        let booking_id = make_id(&env, 0x02);
        let court_id = String::from_str(&env, "Pasig Court A");

        // First booking succeeds
        client.book_slot(&booking_id, &player, &court_owner, &court_id, &1_700_000_100, &amount);

        // Second booking with the SAME id must fail
        let result = client.try_book_slot(
            &booking_id,
            &player,
            &court_owner,
            &court_id,
            &1_700_000_100,
            &amount,
        );

        assert_eq!(
            result,
            Err(Ok(Error::AlreadyBooked)),
            "Duplicate booking_id must be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3 — State verification
    // After book_slot, all fields in persistent storage are correct.
    // -----------------------------------------------------------------------
    #[test]
    fn test_storage_state_after_booking() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _admin, _token, _token_client, token_admin) = setup(&env);

        let player = Address::generate(&env);
        let court_owner = Address::generate(&env);
        let amount: i128 = 25_000_000;
        let slot_ts: u64 = 1_700_005_000;

        token_admin.mint(&player, &amount);

        let booking_id = make_id(&env, 0x03);
        let court_id = String::from_str(&env, "Marikina Court 2");

        client.book_slot(&booking_id, &player, &court_owner, &court_id, &slot_ts, &amount);

        let record = client.get_booking(&booking_id);

        assert_eq!(record.player, player, "Player address must be stored correctly");
        assert_eq!(record.court_owner, court_owner, "Court owner must be stored correctly");
        assert_eq!(record.amount, amount, "Escrowed amount must match");
        assert_eq!(record.slot_timestamp, slot_ts, "Slot timestamp must match");
        assert_eq!(
            record.status,
            BookingStatus::Active,
            "Newly created booking must be Active"
        );
    }

    // -----------------------------------------------------------------------
    // Test 4 — Refund path
    // Admin cancels booking; player receives full refund.
    // -----------------------------------------------------------------------
    #[test]
    fn test_refund_returns_funds_to_player() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _admin, _token, token_client, token_admin) = setup(&env);

        let player = Address::generate(&env);
        let court_owner = Address::generate(&env);
        let amount: i128 = 50_000_000;

        token_admin.mint(&player, &amount);

        let booking_id = make_id(&env, 0x04);
        let court_id = String::from_str(&env, "BGC Court 3");

        client.book_slot(&booking_id, &player, &court_owner, &court_id, &1_700_010_000, &amount);

        let player_balance_before = token_client.balance(&player);
        assert_eq!(player_balance_before, 0, "Funds should be in escrow");

        // Admin refunds
        client.refund_booking(&booking_id);

        assert_eq!(
            token_client.balance(&player),
            amount,
            "Player must receive full refund"
        );

        let record = client.get_booking(&booking_id);
        assert_eq!(record.status, BookingStatus::Refunded);
    }

    // -----------------------------------------------------------------------
    // Test 5 — Unauthorized completion attempt
    // A random third party (not the court owner or admin) cannot complete a booking.
    // -----------------------------------------------------------------------
    #[test]
    fn test_unauthorized_complete_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _admin, _token, _token_client, token_admin) = setup(&env);

        let player = Address::generate(&env);
        let court_owner = Address::generate(&env);
        let random_caller = Address::generate(&env); // neither owner nor admin
        let amount: i128 = 50_000_000;

        token_admin.mint(&player, &amount);

        let booking_id = make_id(&env, 0x05);
        let court_id = String::from_str(&env, "Quezon City Court 5");

        client.book_slot(&booking_id, &player, &court_owner, &court_id, &1_700_020_000, &amount);

        let result = client.try_complete_booking(&booking_id, &random_caller);

        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "Only court owner or admin can complete a booking"
        );
    }
}