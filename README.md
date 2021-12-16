# Project Eclipse _(eclipse)_
> Bridging ecosystems by storing Zero-knowledge proofs of Solana votes on the Aleo blockchain.

[Aleo](https://www.aleo.org/) is a toolkit for private computation that utilizes zero-knowledge proofs stored on a blockchain.

[Solana](https://solana.com/) is a blockchain based on a distributing voting system for validation.

By storing periodic zero-knowledge proofs of the Solana votes on the Aleo blockchain, it is possible to create a two-way bridge between Solana, Aleo, and potentially any other system that is able to verify the proofs.

## Install

Eclipse is written in Rust, and therefore uses the [Rust toolchain](https://www.rust-lang.org/tools/install).

```bash
$ git clone https://github.com/eqlabs/eclipse && cd eclipse
$ cargo build
$ cargo run
```

## Usage

From within the `eclipse` folder:

```bash
$ cargo run
```

You will then see output similar to:

```bash
current network slot: 101273186, processing slot 101273186
found block 97957109 from parent slot 101273185 with 140 transactions
140 transactions decoded
successfully verified signature; adding to slot signatures
successfully verified signature; adding to slot signatures
successfully verified signature; adding to slot signatures
successfully verified signature; adding to slot signatures
successfully verified signature; adding to slot signatures
proof generated for slot 101273186 with 5 votes
```

## Contributing

Please feel free to open issues and pull requests.

## License

Project Eclipse is licensed under the GPLv3, primarily due to copyleft from [SnarkVM](https://github.com/AleoHQ/snarkvm/)