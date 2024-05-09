# CW-ATOMIC-SWAP

![image](./assets/cw-atomic-swap.png)

<p align="center" width="100%">
    <img  height="20" src="https://github.com/0xstepit/cw-atomic-swap/actions/workflows/lint.yml/badge.svg">
    <img height="20" src="https://github.com/0xstepit/cw-atomic-swap/actions/workflows/test.yml/badge.svg">
</p>

`cw-atomic-swap` is a simplified implementation of the [ICS-100](https://github.com/cosmos/ibc/tree/main/spec/app/ics-100-atomic-swap) that
allows two users on the same chain to exchange tokens with an implicit agreement on the the relative price of the two swapped assets.
The contract is a simplified version of ICS-100 because it is made to work on a single and does not allow the execution of trades via IBC. The
system can be viewed as an on-chain Over The Counter (OTC) market.


## How it works


## Interfaces

### Transactions

#### Instantiate

```rust
pub struct InstantiateMsg {
    /// Owner of the smart contract.
    pub owner: Option<String>,
}
```

#### ExecuteMsg

```rust
UpdateConfig {
  /// New contract owner.
  new_owner: String,
}
```

```rust
CreateSwapOrder {
    /// Coin to send.
    coin_in: Coin,
    /// Coin to received.
    coin_out: Coin,
    /// If specified, is the only counterparty accepted in the swap.
    taker: Option<String>,
    /// Timestamp after which the deal expires in seconds.
    timeout: u64,
}
```

```rust
AcceptSwapOrder {
    /// Identifier of the swap order the user wants to match.
    order_id: u64,
    /// The maker associated with the order.
    maker: String,
}
```

```rust
ConfirmSwapOrder {
    /// Identifier of the swap order to confirm.
    order_id: u64,
    /// The maker associated with the order.
    maker: String,
}
```

### Queries

```rust
Config {}
```

```rust
AllSwapOrders {}
```

```rust
SwapOrdersByMaker {
    maker: String
}
```

## Getting Started

These instructions will help you get a copy of the smart contract on your local machine for development and testing purposes.

### Prerequisites

- [CosmWasm](https://github.com/CosmWasm/cosmwasm)
- Rust: [Installation Guide](https://www.rust-lang.org/tools/install)
- Command runner: [just](https://github.com/casey/just)

### Installation

1. Clone the repository and move into project directory:

    ```shell
    git clone https://github.com/0xstepit/cw-atomic-swap.git
    cd cw-atomic-swap
    ```

2. Build the smart contract:

    ```shell
    just optimize
    ```

### Test

```shell
just test
```

### Lint

```shell
just clippy && just fmt
```

### JSON Schema

```shell
just schema
```

## Considerations

## License

This project is licensed under the MIT License - see the LICENSE file for details.
