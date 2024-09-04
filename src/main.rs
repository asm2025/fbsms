mod common;
mod operations;

use anyhow::Result;
use log::{error, info};
use rustmix::{
    io::path::AsPath,
    logging::{log4rs, LogLevel},
    mail::tempmail::TempMail,
    threading::consumer::*,
};
use std::sync::Arc;

use common::*;
use operations::*;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let log_path = ("_logs", "anna.log").as_path();
    let log_level = if is_debug() {
        LogLevel::Debug
    } else {
        LogLevel::Info
    };
    let gaurd = log4rs::init_with(&log_path, log_level, None)?;
    info!("Initializing...");

    match start().await {
        Ok(_) => {
            info!("Done");
        }
        Err(e) => {
            error!("Error: {}", e);
        }
    }

    info!("Shutting down...");
    drop(gaurd);
    Ok(())
}

async fn start() -> Result<()> {
    let threads = num_threads();
    let handler = TaskHandler::new();
    let options = ConsumerOptions::new();
    let consumer = Consumer::<Arc<TempMail>>::with_options(options);
    info!("Starting...");
    info!("Using {} threads", threads);

    for _ in 0..threads {
        consumer.start_async(&handler.clone()).await;
    }

    for _ in 0..1 {
        let email = TempMail::random().await?;
        consumer.enqueue(Arc::new(email));
    }

    consumer.complete();
    consumer.wait_async().await;

    Ok(())
}
