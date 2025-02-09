use chrono::Utc;
use std::io::{self, Write};
use iroh::{protocol::Router, Endpoint};
use iroh_blobs::{net_protocol::Blobs, ticket::BlobTicket};
use tracing_subscriber::{prelude::*, EnvFilter};

// set the RUST_LOG env var to one of {debug,info,warn} to see logging info
pub fn setup_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .try_init()
        .ok();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging();
    println!("Poem Sharing P2P Node");

    // Get username
    let username = loop {
        print!("Please enter your username: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("Error: Username cannot be empty");
            continue;
        }

        if input.len() > 100 {
            println!("Error: Username exceeds 100 characters");
            continue;
        }

        break input;
    };

    // Get poem
    let poem = loop {
        print!("Enter your poem (max 100 characters): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_string();

        if input.is_empty() {
            println!("Error: Poem cannot be empty");
            continue;
        }

        if input.len() > 100 {
            println!("Error: Poem exceeds 100 characters");
            continue;
        }

        break input;
    };

    // create a new node
    let endpoint = Endpoint::builder().bind().await?;
    let builder = Router::builder(endpoint);
    let blobs = Blobs::memory().build(builder.endpoint());
    let builder = builder.accept(iroh_blobs::ALPN, blobs.clone());
    let blobs_client = blobs.client();
    let node = builder.spawn().await?;

    // Create blob content with username and timestamp
    let timestamp = Utc::now().to_rfc3339();
    let content = format!("Author: {}\nTimestamp: {}\nPoem:\n{}", username, timestamp, poem).into_bytes();

    // add some data and remember the hash
    let res = blobs_client.add_bytes(content).await?;

    // create a ticket
    let addr = node.endpoint().node_addr().await?;
    let ticket = BlobTicket::new(addr, res.hash, res.format)?;

    // print some info about the node
    println!("serving hash:    {}", ticket.hash());
    println!("node id:         {}", ticket.node_addr().node_id);
    println!("node listening addresses:");
    for addr in ticket.node_addr().direct_addresses() {
        println!("\t{:?}", addr);
    }
    println!(
        "node relay server url: {:?}",
        ticket
            .node_addr()
            .relay_url()
            .expect("a default relay url should be provided")
            .to_string()
    );
    // print the ticket, containing all above information
    println!("\nin another terminal, run:");
    println!("\t cargo run --example fetch-poem {}", ticket);

    // block until SIGINT is received (ctrl+c)
    tokio::signal::ctrl_c().await?;
    node.shutdown().await?;
    Ok(())
}