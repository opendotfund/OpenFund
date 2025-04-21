# OpenFund
OpenFund - Solana Decentralized Exchange Smart Contracts
Solana
Rust
Anchor
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)
OpenFund is a decentralized exchange (DEX) built on the Solana blockchain, designed for fast, low-cost, and secure token trading. This public repository contains the smart contracts (programs) that power the OpenFund DEX, implemented in Rust using the Anchor framework.
 Overview
OpenFund provides a robust platform for decentralized trading with the following key features:
Automated Market Maker (AMM): Enables liquidity pool-based token swaps with constant product pricing.

Fee Management: Collects and distributes trading fees to liquidity providers and a protocol treasury.

Oracle Integration: Supports price feeds from Pyth and Switchboard for accurate pricing.

Settlement System: Facilitates order creation, execution, and cancellation with escrow-based trading.

Token Management: Supports creation and management of SPL and Token-2022 tokens with metadata.

The contracts leverage Solana’s high-throughput architecture, integrating with native programs like the SPL Token Program, Token-2022 Program, and Associated Token Account Program.
Smart Contract Programs
The repository includes the following Anchor programs:
Core AMM DEX (openfund_dex):
Initializes liquidity pools with token pairs.

Supports adding/removing liquidity and executing token swaps.

Uses a constant product formula (x * y = k) for pricing.

Fee Management (openfund_fee_management):
Configures trading fees (up to 10%) and splits them between liquidity providers and the protocol treasury.

Calculates fees for swaps and collects protocol fees.

Oracle Integration (openfund_oracle):
Integrates with Pyth and Switchboard oracles for real-time price feeds.

Validates price freshness and confidence for secure trading.

Settlement (openfund_settlement):
Manages order creation, cancellation, and execution with escrow accounts.

Supports batch execution and expired order claims with settlement fees (up to 1%).

Token Management (openfund_token_management):
Creates and manages SPL and Token-2022 tokens with metadata (name, symbol, URI).

Initializes token accounts for users and the DEX.

 Current Status
 This repository contains only the basic skeleton of the OpenFund DEX smart contracts, developed several months ago as an early prototype. The current contracts provide foundational functionality but are incomplete, lacking:
Full optimization and performance enhancements.

Comprehensive security audits.

Advanced features like governance, multi-pool support, or Chainlink integration.

Complete implementation of batch operations and expired order claims.

The fully developed and improved version of the smart contracts will be released all at once in a future update. We are actively working on completing the DEX, and updates will be shared via this repository.
 Installation
To explore or test the skeleton contracts, set up the development environment as follows:
Prerequisites
Rust: 1.81.0+ (rustup update stable).

Solana CLI: 1.18.25+ (sh -c "$(curl -sSfL https://release.solana.com/stable/install)").

Anchor: 0.30.1 (cargo install --git https://github.com/coral-xyz/anchor avm --locked --force).

Node.js (optional, for tests): 18+.

A Solana wallet (e.g., Phantom) for Devnet testing.

Steps
Clone the Repository:
bash

git clone https://github.com/opendotfund/OpenFund.git
cd OpenFund

Install Dependencies:
bash

cargo build
anchor build

Configure Solana CLI:
Set the network to Devnet:
bash

solana config set --url https://api.devnet.solana.com

Update Program IDs:
Replace placeholder declare_id! values in each program (e.g., Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS) with your generated program IDs.

Run anchor build to generate new IDs and update Anchor.toml and program files.

Deploy to Devnet:
bash

solana-keygen new
anchor deploy

Run Tests:
Execute the test suite (if included):
bash

anchor test

 Usage
The skeleton contracts are for development and testing purposes only. You can:
Review Code: Explore the programs/ directory for Rust files (e.g., openfund_dex.rs, openfund_fee_management.rs).

Test on Devnet: Deploy to Solana’s Devnet to test AMM swaps, fee collection, oracle price feeds, or order settlement.

Extend Functionality: Use the contracts as a foundation for custom DEX development.

Example: Testing a Swap
Deploy the openfund_dex program to Devnet.

Initialize a liquidity pool with two tokens (e.g., SOL/USDC) using initialize_pool.

Add liquidity via add_liquidity and test a swap with swap.

Use the openfund_fee_management program to calculate and collect fees.

Query the openfund_oracle program for price data to validate the swap.

Note: These contracts are not production-ready. Await the full release for complete functionality.
 Contributing
We welcome contributions, but note that the current skeleton is a work in progress. To contribute:
Fork the repository.

Create a feature branch (git checkout -b feature/your-feature).

Commit changes (git commit -m 'Add feature').

Push to the branch (git push origin feature/your-feature).

Open a pull request.

For major changes, open an issue to discuss your ideas. Contributions to the full release will be prioritized after development is complete.
 License
This project is licensed under the MIT_license. See the LICENSE file for details.
 Contact
For questions, feedback, or collaboration:
GitHub Issues: https://github.com/opendotfund/OpenFund/issues

Email: contact@opendotfund.org (replace with your actual email)

Twitter: @OpenDotFund (replace with your actual handle)

Stay tuned for the full release of the OpenFund DEX smart contracts!

