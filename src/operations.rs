use anyhow::Result;
use colored::Colorize;
use humantime::format_duration;
use log::{debug, error, info, warn};
use rustmix::{
    io::{
        file::{create_with, FileOpenOptions},
        path::AsPath,
    },
    mail::tempmail::TempMail,
    threading::{consumer::*, AsyncTaskDelegation, TaskDelegationBase, TaskResult},
    web::{build_client, url::AsUrl},
};
use std::{
    collections::HashMap,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use crate::common::*;

const OUT_DIR: &'static str = "out";

#[derive(Debug, Clone)]
pub struct TaskHandler {
    started: Arc<Mutex<Option<Instant>>>,
    client: Arc<reqwest::Client>,
    save_request: Arc<AtomicBool>,
}

impl TaskHandler {
    pub fn new() -> Self {
        TaskHandler {
            started: Arc::new(Mutex::new(None)),
            client: Arc::new(
                build_client()
                    .timeout(Duration::from_secs(5))
                    .build()
                    .unwrap(),
            ),
            save_request: Arc::new(AtomicBool::new(false)),
        }
    }

    fn save_request(&self, step: &usize, text: &str) {
        if !self.save_request.load(Ordering::SeqCst) {
            return;
        }

        let file_path = (OUT_DIR, format!("{:02}.html", &step).as_str()).as_path();
        info!("Step {}: response is written to '{}'", &step, &file_path);
        let file = create_with(file_path, FileOpenOptions::Truncate).unwrap();
        write!(&file, "{}", &text).unwrap();
    }
}

impl TaskDelegationBase<Consumer<Arc<TempMail>>, Arc<TempMail>> for TaskHandler {
    fn on_started(&self, pc: &Consumer<Arc<TempMail>>) {
        info!("Started processing tasks...");

        let mut started = self.started.lock().unwrap();
        *started = Some(Instant::now());
        let consumers = pc.consumers();
        debug!("Using {} consumer(s)", consumers);

        if is_debug() && consumers == 1 {
            self.save_request.store(true, Ordering::SeqCst);
            debug!("Responses will be written to directory '{}'", OUT_DIR);
        }
    }

    fn on_completed(
        &self,
        _pc: &Consumer<Arc<TempMail>>,
        item: &Arc<TempMail>,
        result: &TaskResult,
    ) -> bool {
        match result {
            TaskResult::Success => info!("{} -> {}", item.address(), "Ok".green()),
            TaskResult::Error(msg) => error!("{} -> {} {}", item.address(), "Error:".red(), msg),
            TaskResult::TimedOut => warn!("{} -> {}", item.address(), "Timedout".magenta()),
            _ => {}
        }
        true
    }

    fn on_finished(&self, _pc: &Consumer<Arc<TempMail>>) {
        let started = self.started.lock().unwrap();
        let elapsed = started.unwrap().elapsed();
        info!(
            "Finished processing tasks. Took {}",
            format_duration(elapsed)
        );
    }
}

impl AsyncTaskDelegation<Consumer<Arc<TempMail>>, Arc<TempMail>> for TaskHandler {
    async fn process(
        &self,
        _td: &Consumer<Arc<TempMail>>,
        item: &Arc<TempMail>,
    ) -> Result<TaskResult> {
        const ID_BASE_URL: &'static str = "https://id.anna.money/";
        const API_BASE_URL: &'static str = "https://api.anna.money/";

        info!("Processing {}", item.address());
        let email_address = item.address();
        debug!("Using email address: {}", &email_address);

        let url = (
            ID_BASE_URL,
            "authorize?client_id=universal&redirect_uri=https://anna.money&screen=signup",
        )
            .as_url()
            .unwrap();

        let mut step = 1;
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(e) => {
                if e.is_timeout() {
                    return Ok(TaskResult::TimedOut);
                } else {
                    return Ok(TaskResult::Error(e.to_string()));
                }
            }
        };

        if !response.status().is_success() {
            return Ok(TaskResult::Error(format!(
                "Failed to process email '{}'. Status: {}, Error: {}",
                &email_address,
                response.status(),
                response.error_for_status().unwrap().text().await.unwrap()
            )));
        }

        let query: HashMap<_, _> = response.url().query_pairs().into_owned().collect();
        info!("Step {}: query: {:?}", &step, &query);

        let mid = query.get("mid").unwrap();

        let text = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                return Ok(TaskResult::Error(e.to_string()));
            }
        };

        if text.is_empty() {
            return Ok(TaskResult::Error("Empty response".to_string()));
        }

        self.save_request(&step, &text);

        //step += 1;
        Ok(TaskResult::Success)
    }
}
