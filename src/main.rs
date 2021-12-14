#![warn(missing_debug_implementations, rust_2018_idioms)]

use {
    clap::{crate_description, crate_name, crate_version, App, Arg},
    solana_client::rpc_client::RpcClient,
    std::{str::FromStr, time::Duration},
    url::Url,
};

mod aleo;
mod eclipse;
mod solana;

fn main() {
    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .arg(
            Arg::with_name("block_height")
                .long("block_height")
                .value_name("BLOCK HEIGHT")
                .takes_value(true)
                .validator(is_u64)
                .help("Initial block height to process from"),
        )
        .arg(
            Arg::with_name("json_rpc_url")
                .long("url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .default_value("https://api.devnet.solana.com")
                .help("JSON RPC URL for the cluster"),
        )
        .arg(
            Arg::with_name("interval")
                .long("interval")
                .value_name("SECONDS")
                .takes_value(true)
                .default_value("10")
                .validator(is_u64)
                .help("Block poll interval seconds"),
        )
        .arg(
            Arg::with_name("threshold")
                .long("threshold")
                .value_name("COUNT")
                .takes_value(true)
                .default_value("5")
                .validator(is_u64)
                .help("How many votes a slot must have before proof is generated for it"),
        )
        .get_matches();

    let url = matches.value_of("json_rpc_url").unwrap();
    let poll_interval = u64::from_str(matches.value_of("interval").unwrap()).unwrap();
    let confirmation_threshold = usize::from_str(matches.value_of("threshold").unwrap()).unwrap();
    let client = RpcClient::new(url.to_string());

    let start_slot = match matches.value_of("block_height") {
        Some(bh) => u64::from_str(bh).unwrap(),
        None => 0,
    };

    let proof_generator = aleo::DummyProofGenerator {};
    let vote_collector = solana::VoteCollector::new(confirmation_threshold, proof_generator);
    let mut block_processor = solana::BlockProcessor::new(client, vote_collector);

    block_processor.poll_slot_votes(start_slot, Duration::from_secs(poll_interval));
}

fn is_url(string: String) -> Result<(), String> {
    match Url::parse(&string) {
        Ok(_) => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn is_u64(string: String) -> Result<(), String> {
    match u64::from_str(&string) {
        Ok(_) => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}
