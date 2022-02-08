# Eclipse

This repository works with the Eclipse version of the Solana node.

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

#### Onchain Program

Now compile the onchain program in this repository in `/program` by `cargo build-bpf`.
This will output a file in `/program/target/deploy/eclipse_onchain_program.so`

Then use the previously compiled Eclipse version of the solana binary to deploy the program.
In the _Solana_ repository:

```sh
./bin/solana program deploy <path-to-the-eclipse_onchain_program.so>
```

Take a note of the program-id logged here for next step.

#### Eclipse Service

Run eclipse service by

```sh
./target/debug/aleo-to-solana --solana_keypair <default-path-to-solana-test-verifier-config-keypair> verify_proofs --eclipse_program_id <Eclipse-onchain-program-id>
```

This will run the eclipse service continuously.
We have printed out where the verification results are stored, take a note of it for the next step.

#### Checking

You can check if an Aleo tx has been verified by using the Eclipse version of the solana binary.
In the _Solana_ repository:

```sh
./bin/solana account <account-where-verification-results-stored>
```
