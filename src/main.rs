use chrono::Utc;
use dialoguer::{theme::SimpleTheme, Input};
use futures_lite::StreamExt;
use iroh::{protocol::Router, Endpoint};
use iroh_blobs::{store::Store as BlobStore, net_protocol::Blobs, ALPN as BLOBS_ALPN};
use iroh_docs::{DocTicket, engine::LiveEvent, protocol::Docs, ALPN as DOCS_ALPN};
use iroh_gossip::{net::Gossip, ALPN as GOSSIP_ALPN};
use std::{sync::Arc, str::FromStr};
use tracing_subscriber::{prelude::*, EnvFilter};
use iroh_blobs::util::local_pool::LocalPool;

// Set up logging for the application
fn setup_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .try_init()
        .ok();
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    setup_logging();
    println!("Poem Sharing P2P Node");

    // Create an iroh endpoint
    let endpoint = Endpoint::builder().discovery_n0().bind().await?;

    // Create an in-memory blob store
    let blobs = Blobs::memory().build(&endpoint);

    // Turn on the "rpc" feature if you need to create blobs and tags clients
    let blobs_client = blobs.client();
    let tags_client = blobs_client.tags();

    // Create a router builder
    let mut builder = Router::builder(endpoint.clone());

    // Build the gossip protocol
    let gossip = Gossip::builder().spawn(builder.endpoint().clone()).await?;

    // Build the docs protocol
    let docs = Docs::<BlobStore>::memory().spawn(&blobs, &gossip).await?;

    // Setup router
    let router = builder
        .accept(BLOBS_ALPN, blobs.clone())
        .accept(GOSSIP_ALPN, gossip.clone())
        .accept(DOCS_ALPN, docs.clone())
        .spawn()
        .await?;

    // Initialize the Iroh node
    let iroh = Iroh {
        _local_pool: Arc::new(LocalPool::default()),
        router,
        gossip: Arc::new(gossip),
        blobs: blobs_client.clone(),
        docs,
    };

    println!("Author: {}", iroh.docs.list_authors().await?.fmt_short());

    // Start the menu loop for user interaction
    tokio::spawn(menu_loop(iroh.clone())).await?;
    Ok(())
}

// Menu loop for user commands
async fn menu_loop(node: Iroh) {
    loop {
        // Prompt user for a command
        match Input::<String>::with_theme(&SimpleTheme)
            .with_prompt("Command (create/join):")
            .interact_text()
            .unwrap()
            .as_str()
        {
            "create" => handle_create(&node).await, // Handle document creation
            "join" => handle_join(&node).await,     // Handle joining a document
            _ => println!("Invalid command. Please enter 'create' or 'join'."),
        }
    }
}

// Handle creating a new document
async fn handle_create(node: &Iroh) {
    // Create a new document
    let temp_doc = node.docs.new_author().await.unwrap();
    // Share the document and get a ticket
    let ticket = temp_doc
        .share(
            ShareMode::Write,
            AddrInfoOptions::RelayAndAddresses,
        )
        .await
        .unwrap();

    println!("Share this ticket with peers: {}", ticket);

    // Import the document for synchronization
    let poem_doc = node.docs.import(ticket.clone()).await.unwrap();
    poem_doc.set_bytes(
        node.docs.list_authors().await.unwrap(),
        "poem-ticket",
        ticket.to_string(),
    ).await.unwrap();

    // Start the poem loop for real-time updates
    tokio::spawn(poem_loop(poem_doc.clone(), node.clone()));
    loop {
        // Prompt user for poem input
        let line = Input::with_theme(&SimpleTheme)
            .with_prompt("Poem:")
            .interact_text()
            .unwrap();
        let timestamp = Utc::now().timestamp_micros().to_string();
        poem_doc.set_bytes(
            node.docs.list_authors().await.unwrap(),
            timestamp,
            line,
        ).await.unwrap();
    }
}

// Handle joining an existing document
async fn handle_join(node: &Iroh) {
    // Prompt user for a ticket to join
    let join_ticket: String = Input::<String>::with_theme(&SimpleTheme)
        .with_prompt("Ticket:")
        .interact_text()
        .unwrap()
        .trim()
        .to_string();

    // Convert the string to a DocTicket
    let doc_ticket = DocTicket::from_str(&join_ticket).expect("Invalid ticket format");

    // Import the document using the ticket
    let poem_doc = node.docs.import(doc_ticket).await.unwrap();
    tokio::spawn(poem_loop(poem_doc.clone(), node.clone()));
    loop {
        // Prompt user for poem input
        let line: String = Input::<String>::with_theme(&SimpleTheme)
            .with_prompt("Poem:")
            .interact_text()
            .unwrap();
        let timestamp = Utc::now().timestamp_micros().to_string();
        poem_doc.set_bytes(
            node.docs.list_authors().await.unwrap(),
            timestamp,
            line,
        ).await.unwrap();
    }
}

// Loop to handle real-time updates to the poem document
async fn poem_loop(poem_doc: Docs<BlobStore>, iroh: Iroh) {
    let mut sub = poem_doc.subscribe().await.unwrap();
    let blobs = iroh.blobs.clone();

    while let Ok(Some(LiveEvent::InsertRemote { from, entry, .. })) = sub.try_next().await {
        let mut message_content = blobs.read_to_bytes(entry.content_hash()).await;

        // Retry up to 3 times if initial read fails
        for _ in 0..3 {
            if message_content.is_ok() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            message_content = blobs.read_to_bytes(entry.content_hash()).await;
        }

        // Print message if we successfully got the content
        if let Ok(content) = message_content {
            println!(
                "{}: {}",
                from.fmt_short(),
                String::from_utf8(content.into()).unwrap()
            );
        }
    }
}

// Struct to manage the Iroh node and its protocols
#[derive(Clone, Debug)]
pub(crate) struct Iroh {
    _local_pool: Arc<LocalPool>,
    router: Router,
    pub(crate) gossip: Arc<Gossip>,
    pub(crate) blobs: BlobStore,
    pub(crate) docs: Docs<BlobStore>,
}