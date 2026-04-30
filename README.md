CourtLink

Book a basketball court slot on-chain. No middlemen. No double-bookings. Guaranteed.


Problem
A group of students in Quezon City wants to play basketball but arrives at the local court only to find it was double-booked by a "middleman," resulting in lost time and a wasted cash deposit.
Solution
A mobile web app with a map interface where players book slots via a Soroban Smart Contract that holds the payment in escrow and issues an on-chain "Booking Pass," ensuring the slot is guaranteed — or the money is refunded automatically.

Vision & Purpose
CourtLink transforms physical court space into a verifiable Real-World Asset (RWA) on Stellar. By eliminating the middleman, it solves double-booking fraud endemic to local recreational leagues across Southeast Asia. Court owners gain a trustless, instant-settlement payment layer in XLM/USDC that works where traditional banking often fails. Players gain peace of mind: their slot is either guaranteed or their money is back — enforced by code, not promises.

Stellar Features Used
FeatureRoleSoroban Smart ContractsEscrow logic, booking state machine, refund rulesXLM / USDC (SAC)Payment token held in escrowContract EventsSLT_BKND event triggers frontend map to mark slot unavailableLocal Anchor (optional)ArmenoTech / GCash bridge converts PHP → USDC on-chain

MVP Core Feature
Player selects 2 PM slot on map
  → Sends 50 XLM to CourtLink contract  (book_slot)
  → Contract emits SLT_BKND event
  → Funds held in escrow until game ends
  → Court owner calls complete_booking  → funds released
  → OR admin calls refund_booking       → player refunded

Suggested MVP Timeline
DayMilestone1Scaffold Soroban contract; implement book_slot + complete_booking2Write & pass all 5 tests; deploy to Testnet3Build map UI (Leaflet.js); integrate Freighter wallet4Wire frontend → contract calls via Stellar SDK5Polish, record demo video, submit

Prerequisites

Rust >= 1.74 with wasm32-unknown-unknown target

bash  rustup target add wasm32-unknown-unknown

Stellar CLI >= 22.0.0

bash  cargo install --locked stellar-cli --features opt

Node.js >= 18 (for frontend, optional at contract stage)


Build
bash# From the repo root
stellar contract build
Output WASM lands at:
target/wasm32-unknown-unknown/release/courtlink.wasm

Test
bashcargo test
All 5 tests should pass:

test_book_and_complete_happy_path
test_duplicate_booking_rejected
test_storage_state_after_booking
test_refund_returns_funds_to_player
test_unauthorized_complete_rejected


Deploy to Testnet
bash# 1. Generate a testnet identity (skip if you already have one)
stellar keys generate --global alice --network testnet

# 2. Fund it from Friendbot
stellar keys fund alice --network testnet

# 3. Deploy the contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/courtlink.wasm \
  --source alice \
  --network testnet
# → Returns CONTRACT_ID

# 4. Initialise the contract
stellar contract invoke \
  --id CONTRACT_ID \
  --source alice \
  --network testnet \
  -- initialize \
  --admin ADMIN_ADDRESS \
  --token XLM_SAC_ADDRESS

Sample CLI Invocations
book_slot — Player books the 2 PM court slot
bashstellar contract invoke \
  --id CONTRACT_ID \
  --source player_alice \
  --network testnet \
  -- book_slot \
  --booking_id aabbccdd00000000000000000000000000000000000000000000000000000000 \
  --player GPLAYER_ADDRESS \
  --court_owner GOWNER_ADDRESS \
  --court_id "Cubao Court 1" \
  --slot_timestamp 1700000000 \
  --amount 500000000
complete_booking — Court owner releases escrow after game
bashstellar contract invoke \
  --id CONTRACT_ID \
  --source court_owner \
  --network testnet \
  -- complete_booking \
  --booking_id aabbccdd00000000000000000000000000000000000000000000000000000000 \
  --caller GOWNER_ADDRESS
refund_booking — Admin refunds a cancelled slot
bashstellar contract invoke \
  --id CONTRACT_ID \
  --source admin \
  --network testnet \
  -- refund_booking \
  --booking_id aabbccdd00000000000000000000000000000000000000000000000000000000
get_booking — View a booking record
bashstellar contract invoke \
  --id CONTRACT_ID \
  --network testnet \
  -- get_booking \
  --booking_id aabbccdd00000000000000000000000000000000000000000000000000000000

Optional Edge: GCash/Maya Anchor Bridge
Connect CourtLink to ArmenoTech (a Philippine Stellar Anchor).
Players pay in PHP via GCash or Maya → anchor converts to USDC on Stellar → funds the escrow automatically. The blockchain is completely invisible to the end user.

Repository Structure
courtlink/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs      ← Soroban contract
    └── test.rs     ← 5 unit tests

License
MIT © 2025 CourtLink Contributors
Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.