# Batch Auction Contract

This project implements a smart contract for a batch auction system. Users can deposit tokens, start an auction, end the auction, and claim their tokens based on their share of deposits.

## Features

- **Initialize Contract**: Set up the initial state of the contract with auction admin, project token, and paired token information.
- **Deposit Tokens**: Users can deposit tokens into the auction.
- **Begin Auction**: The auction admin can start an auction by sending a specified amount of project tokens.
- **End Auction**: The auction admin can end the auction, transferring the total deposited tokens to the admin.
- **Claim Tokens**: Users can claim their share of project tokens based on their deposits after the auction ends.

## Contract Structure

### State

The contract state is stored in the `State` struct and includes:
- `auction_admin`: Address of the auction administrator.
- `project_snip_contract`: Address of the project token contract.
- `project_snip_hash`: Hash of the project token contract.
- `paired_snip_contract`: Address of the paired token contract.
- `paired_snip_hash`: Hash of the paired token contract.
- `auction_amount`: Total amount of project tokens available for auction.
- `total_deposits`: Total amount of paired tokens deposited.
- `auction_active`: Boolean indicating if the auction is active.

### Messages

The contract supports the following messages:
- `InstantiateMsg`: Initializes the contract state.
- `ExecuteMsg`: Executes various functions:
  - `Claim {}`: Claims project tokens based on deposits.
  - `EndAuction {}`: Ends the auction and transfers deposited tokens to the admin.
  - `Receive {}`: Handles incoming token transfers and parses them into specific actions (`Deposit {}`, `BeginAuction {}`).

### Functions

- `instantiate`: Sets up the initial contract state and registers the contract to receive tokens.
- `execute`: Handles the execution of different messages.
- `execute_claim`: Allows users to claim their project tokens after the auction ends.
- `execute_end_auction`: Ends the auction and transfers deposited tokens to the admin.
- `execute_receive`: Parses incoming token transfers into deposit or auction start actions.
- `receive_deposit`: Handles token deposits from users.
- `receive_begin_auction`: Starts the auction by the admin.

## Setup

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [CosmWasm](https://www.cosmwasm.com/docs/getting-started/installation)

### Building the Contract

Clone the repository and navigate to the project directory:

```sh
git clone <repository-url>
cd <project-directory>
```

Build the contract:

```sh
cargo wasm
```

### Running Tests

Run the contract tests:

```sh
cargo test
```

## Usage

### Instantiate the Contract

Instantiate the contract with the initial state:

```json
{
  "auction_admin": "<admin-address>",
  "project_snip_contract": "<project-snip-contract-address>",
  "project_snip_hash": "<project-snip-contract-hash>",
  "paired_snip_contract": "<paired-snip-contract-address>",
  "paired_snip_hash": "<paired-snip-contract-hash>"
}
```

### Execute Messages

- **Deposit Tokens**: Users can deposit paired tokens by sending a `ReceiveMsg::Deposit {}` message along with the token transfer.
- **Begin Auction**: The admin can start the auction by sending a `ReceiveMsg::BeginAuction {}` message along with the project token transfer.
- **End Auction**: The admin can end the auction by sending an `ExecuteMsg::EndAuction {}` message.
- **Claim Tokens**: Users can claim their project tokens by sending an `ExecuteMsg::Claim {}` message after the auction ends.

## Example Interactions

### Deposit Tokens

```json
{
  "send": {
    "contract": "<contract-address>",
    "amount": "<deposit-amount>",
    "msg": "{\"deposit\":{}}"
  }
}
```

### Begin Auction

```json
{
  "send": {
    "contract": "<contract-address>",
    "amount": "<auction-amount>",
    "msg": "{\"begin_auction\":{}}"
  }
}
```

### End Auction

```json
{
  "end_auction": {}
}
```

### Claim Tokens

```json
{
  "claim": {}
}
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
