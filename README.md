# Project Eclipse _(eclipse)_
> Bridging ecosystems by storing Zero-knowledge proofs of Solana votes on the Aleo blockchain.

[Aleo](https://www.aleo.org/) is a toolkit for private computation that utilizes zero-knowledge proofs stored on a blockchain.

[Solana](https://solana.com/) is a blockchain based on a distributing voting system for validation.

By storing periodic zero-knowledge proofs of the Solana votes on the Aleo blockchain, it is possible to create a two-way bridge between Solana, Aleo, and potentially any other system that is able to verify the proofs.

## Install

Eclipse is written in Rust, and therefore uses the [Rust toolchain](https://www.rust-lang.org/tools/install).

## To Run Aleo-to-Solana

### Solana Node

We need to compile the Eclipse version of the Solana Node.
For PoC purpose, the easiest way is to compile the `Solana-test-validator` binary.
In our Solana forked reposity root, run

```sh
./scripts/cargo-install-all.sh .
```

Then you can run the `Solana-test-validator` by

```sh
./bin/solana-test-validator -r --log
```

Details found [here](https://docs.solana.com/developing/test-validator) on the `solana-test-validator`.

**Note**: help trace by setting flag:

`export RUST_LOG=solana_runtime::system_instruction_processor=trace,solana_runtime::message_processor=trace,solana_bpf_loader=trace,solana_rbpf=trace`

#### Onchain Programs

Now compile the on-chain programs. Under the `/programs/aleo-verifier` and
`/programs/uploader` by `cargo build-bpf --sdk=<path to solana>/sdk/bpf`. This will output files in
`/target/deploy/eclipse_aleo_verifier.so` and `/target/deploy/eclipse_uploader.so`

Then use the previously compiled Eclipse version of the solana binary to deploy the program.
In the _Solana_ repository:

```sh
./bin/solana program deploy <path-to-the-eclipse_aleo_verifier.so>
./bin/solana program deploy <path-to-the-eclipse_uploader.so>
```

Take a note of the program-id logged here for next step.

#### Eclipse Service

Run eclipse service by

```sh

```bash
$ git clone https://github.com/eqlabs/eclipse && cd eclipse/service/src/aleo-to-solana
$ cargo build
$ ./target/debug/aleo-to-solana \
--author_keypair <default-path-to-solana-test-verifier-config-keypair> \
--payer_keypair <default-path-to-solana-test-verifier-config-keypair> \
verify_proofs \
--uploader_program_id <eclipse-onchain-uploader-program-id> \
--verifier_program_id <eclipse-onchain-aleo-verifier-program-id>
```

This will run the eclipse service continuously.
We have printed out where the verification results are stored, take a note of it for the next step.

#### Checking

You can check if an Aleo tx has been verified by using the Eclipse version of the solana binary.
In the _Solana_ repository:

```sh
./bin/solana account <account-where-verification-results-stored>
```

## Contributing

Please feel free to open issues and pull requests.

## License

Project Eclipse is licensed under the GPLv3, primarily due to copyleft from [SnarkVM](https://github.com/AleoHQ/snarkvm/)
