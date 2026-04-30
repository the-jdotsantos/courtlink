#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    token, Address, BytesN, Env, String, Symbol,
};
 
// ---------------------------------------------------------------------------
// Storage key types
// ---------------------------------------------------------------------------
 
/// Top-level storage keys for instance (admin config) data.
#[contracttype]
#[derive(Clone)]
pub enum ConfigKey {
    Admin,
    Token,
}
 
/// Per-booking storage key — keyed by the booking ID (32-byte hash).
#[contracttype]
#[derive(Clone)]
pub enum BookingKey {
    Booking(BytesN<32>),
}
 
// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------
 
/// Status of a court booking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BookingStatus {
    /// Payment held in escrow; slot is reserved.
    Active,
    /// Game ended; funds released to court owner.
    Completed,
    /// Booking was cancelled; funds refunded to player.
    Refunded,
}
 
/// A single booking record stored on-chain.
#[contracttype]
#[derive(Clone)]
pub struct Booking {
    /// The player who made the booking.
    pub player: Address,
    /// The court owner who will receive the payment.
    pub court_owner: Address,
    /// Court identifier (e.g. "Cubao Court 1").
    pub court_id: String,
    /// Booked slot as a Unix timestamp (start time).
    pub slot_timestamp: u64,
    /// Amount (in stroops) held in escrow.
    pub amount: i128,
    /// Current status of this booking.
    pub status: BookingStatus,
}
 
// ---------------------------------------------------------------------------
// Contract errors
// ---------------------------------------------------------------------------
 
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    /// A booking with this ID already exists.
    AlreadyBooked = 1,
    /// Booking ID not found in storage.
    NotFound = 2,
    /// Caller is not authorised to perform this action.
    Unauthorized = 3,
    /// The booking is not in the required state for this action.
    InvalidStatus = 4,
    /// Amount must be greater than zero.
    InvalidAmount = 5,
}
 
// ---------------------------------------------------------------------------
// Event symbols
// ---------------------------------------------------------------------------
const SLOT_BOOKED: Symbol = symbol_short!("SLT_BKND");
const SLOT_DONE: Symbol = symbol_short!("SLT_DONE");
const SLOT_RFND: Symbol = symbol_short!("SLT_RFND");
 
// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------
 
#[contract]
pub struct CourtLink;
 
#[contractimpl]
impl CourtLink {
    // -----------------------------------------------------------------------
    // initialize
    // -----------------------------------------------------------------------
 
    /// One-time setup: store admin and the payment token (XLM/USDC SAC).
    pub fn initialize(env: Env, admin: Address, token: Address) {
        if env.storage().instance().has(&ConfigKey::Admin) {
            panic!("already initialised");
        }
        env.storage().instance().set(&ConfigKey::Admin, &admin);
        env.storage().instance().set(&ConfigKey::Token, &token);
    }
 
    // -----------------------------------------------------------------------
    // book_slot  — MVP core function
    // -----------------------------------------------------------------------
 
    /// Player books a court slot.
    ///
    /// Flow:
    ///   1. Player calls `book_slot` with a unique booking_id, court details, slot time, and amount.
    ///   2. Contract pulls `amount` tokens from the player into its own escrow address.
    ///   3. Booking record is stored on-chain.
    ///   4. `SLT_BKND` event is emitted so the frontend map updates availability.
    pub fn book_slot(
        env: Env,
        booking_id: BytesN<32>,
        player: Address,
        court_owner: Address,
        court_id: String,
        slot_timestamp: u64,
        amount: i128,
    ) -> Result<(), Error> {
        // Player must sign this transaction.
        player.require_auth();
 
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
 
        // Reject duplicate bookings with the same ID.
        if env
            .storage()
            .persistent()
            .has(&BookingKey::Booking(booking_id.clone()))
        {
            return Err(Error::AlreadyBooked);
        }
 
        // Pull funds from the player's wallet into contract escrow.
        let token_addr: Address = env.storage().instance().get(&ConfigKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(&player, &env.current_contract_address(), &amount);
 
        // Persist the booking record.
        let booking = Booking {
            player: player.clone(),
            court_owner,
            court_id,
            slot_timestamp,
            amount,
            status: BookingStatus::Active,
        };
        env.storage()
            .persistent()
            .set(&BookingKey::Booking(booking_id.clone()), &booking);
 
        // Emit event: off-chain listeners update the court availability map.
        env.events()
            .publish((SLOT_BOOKED, player), (booking_id, slot_timestamp, amount));
 
        Ok(())
    }
 
    // -----------------------------------------------------------------------
    // complete_booking
    // -----------------------------------------------------------------------
 
    /// Court owner (or admin) marks the game as done; releases escrow to owner.
    ///
    /// Only the court owner or the contract admin may call this.
    /// Emits `SLT_DONE` so the map shows the slot as available again.
    pub fn complete_booking(
        env: Env,
        booking_id: BytesN<32>,
        caller: Address,
    ) -> Result<(), Error> {
        caller.require_auth();
 
        let mut booking: Booking = env
            .storage()
            .persistent()
            .get(&BookingKey::Booking(booking_id.clone()))
            .ok_or(Error::NotFound)?;
 
        // Only the court owner or admin can release funds.
        let admin: Address = env.storage().instance().get(&ConfigKey::Admin).unwrap();
        if caller != booking.court_owner && caller != admin {
            return Err(Error::Unauthorized);
        }
 
        // Booking must be Active to complete.
        if booking.status != BookingStatus::Active {
            return Err(Error::InvalidStatus);
        }
 
        // Release escrowed funds to the court owner.
        let token_addr: Address = env.storage().instance().get(&ConfigKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(
            &env.current_contract_address(),
            &booking.court_owner,
            &booking.amount,
        );
 
        booking.status = BookingStatus::Completed;
        env.storage()
            .persistent()
            .set(&BookingKey::Booking(booking_id.clone()), &booking);
 
        env.events()
            .publish((SLOT_DONE, booking.court_owner), booking_id);
 
        Ok(())
    }
 
    // -----------------------------------------------------------------------
    // refund_booking
    // -----------------------------------------------------------------------
 
    /// Admin cancels a booking and refunds the player in full.
    ///
    /// Use case: court cancels the session (maintenance, weather, etc.).
    /// Emits `SLT_RFND` so the map slot re-opens immediately.
    pub fn refund_booking(
        env: Env,
        booking_id: BytesN<32>,
    ) -> Result<(), Error> {
        // Only admin can trigger refunds.
        let admin: Address = env.storage().instance().get(&ConfigKey::Admin).unwrap();
        admin.require_auth();
 
        let mut booking: Booking = env
            .storage()
            .persistent()
            .get(&BookingKey::Booking(booking_id.clone()))
            .ok_or(Error::NotFound)?;
 
        if booking.status != BookingStatus::Active {
            return Err(Error::InvalidStatus);
        }
 
        // Return funds to the player.
        let token_addr: Address = env.storage().instance().get(&ConfigKey::Token).unwrap();
        let token_client = token::Client::new(&env, &token_addr);
        token_client.transfer(
            &env.current_contract_address(),
            &booking.player,
            &booking.amount,
        );
 
        booking.status = BookingStatus::Refunded;
        env.storage()
            .persistent()
            .set(&BookingKey::Booking(booking_id.clone()), &booking);
 
        env.events()
            .publish((SLOT_RFND, booking.player), booking_id);
 
        Ok(())
    }
 
    // -----------------------------------------------------------------------
    // View helpers
    // -----------------------------------------------------------------------
 
    /// Return the full booking record.
    pub fn get_booking(env: Env, booking_id: BytesN<32>) -> Result<Booking, Error> {
        env.storage()
            .persistent()
            .get(&BookingKey::Booking(booking_id))
            .ok_or(Error::NotFound)
    }
 
    /// Return the admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&ConfigKey::Admin).unwrap()
    }
}
 


mod test;