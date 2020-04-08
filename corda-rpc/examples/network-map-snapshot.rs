use structopt::StructOpt;
use tokio;

use corda_rpc::{Client, NetworkMapSnapshot, Rpc};

#[tokio::main]
async fn main() {
    let options = Options::from_args();

    let mut client = Client::new(
        &options.address,
        options.user,
        &options.password,
        "corda-rpc".into(),
    )
    .await
    .unwrap();

    let rpc = NetworkMapSnapshot;
    let frame = client.call(&rpc).await.unwrap();
    let response = rpc.response(&frame).unwrap();
    println!("{:#?}", response);
}

#[derive(Debug, StructOpt)]
struct Options {
    #[structopt(short, long)]
    user: String,
    #[structopt(short, long)]
    password: String,
    address: String,
}
