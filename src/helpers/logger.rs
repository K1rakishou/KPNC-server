use std::fmt;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, DateTime, Local, Timelike, TimeZone, Utc};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

use crate::model::database::db::Database;

pub struct Logger {
    is_dev_build: bool,
    sender: UnboundedSender<LogLine>
}

static mut LOGGER: Option<Logger> = None;

pub fn init_logger(is_dev_build: bool, database: Option<Arc<Database>>) {
    // We init the logger only once at the very beginning so it should be fine
    unsafe { LOGGER = Some(Logger::new(is_dev_build, database)); }
}

fn logger() -> &'static Logger {
    return unsafe { LOGGER.as_ref().unwrap() };
}

impl Logger {
    pub fn new(is_dev_build: bool, database: Option<Arc<Database>>) -> Logger {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<LogLine>();

        tokio::spawn(async move {
            Self::process_logs(is_dev_build, database, receiver).await;
        });

        return Self { is_dev_build, sender };
    }

    async fn process_logs(
        is_dev_build: bool,
        database: Option<Arc<Database>>,
        mut receiver: UnboundedReceiver<LogLine>
    ) {
        let unsent_logs = Arc::new(Mutex::new(Vec::<LogLine>::with_capacity(128)));

        let database_cloned = database.clone();
        let unsent_logs_cloned = unsent_logs.clone();

        tokio::spawn(async move {
            Self::store_logs_in_database(&database_cloned, unsent_logs_cloned).await
        });

        loop {
            let log_line = receiver.recv().await;
            if log_line.is_none() {
                break;
            }

            let log_line = log_line.unwrap();

            // Only print logs to console when is_dev_build is true. In production version only
            // store logs into the database since we won't be able to see them anyway.
            if is_dev_build {
                let local_time: DateTime<Local> = DateTime::from(log_line.date_time);

                let date_time = format!(
                    "{}-{:02}-{:02} {:02}-{:02}-{:02}.{:03}",
                    local_time.year(),
                    local_time.month(),
                    local_time.day(),
                    local_time.hour(),
                    local_time.minute(),
                    local_time.second(),
                    local_time.timestamp_millis() % 1000,
                );

                let formatted_log = format!(
                    "{} [{}] {}@{} -- {}",
                    log_line.log_level,
                    date_time,
                    log_line.target,
                    log_line.thread_id,
                    log_line.arguments
                );

                if log_line.log_level == LogLevel::Info {
                    println!("{}", formatted_log);
                } else {
                    eprintln!("{}", formatted_log);
                }
            }

            {
                unsent_logs.lock().await.push(log_line);
            }
        }
    }

    async fn store_logs_in_database(
        database_cloned: &Option<Arc<Database>>,
        unsent_logs_cloned: Arc<Mutex<Vec<LogLine>>>
    ) {
        if database_cloned.is_none() {
            println!("Database was not passed into the logger, exiting store_logs_in_database()");
            return;
        }

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let unsent_logs_copy = {
                let mut unsent_logs_locked = unsent_logs_cloned.lock().await;
                let unsent_logs_copy = unsent_logs_locked.iter()
                    .map(|value| value.clone())
                    .collect::<Vec<LogLine>>();

                unsent_logs_locked.clear();
                unsent_logs_copy
            };

            if unsent_logs_copy.is_empty() {
                continue;
            }

            let result = Self::delete_old_logs_from_database(
                &database_cloned.as_ref().unwrap().clone()
            ).await;

            if result.is_err() {
                let error = result.err().unwrap();
                println!("Failed to delete old logs from the database, error: {}", error);
            } else {
                let deleted = result.unwrap();
                println!("Deleted {} logs from database", deleted);
            }

            let result = Self::store_logs_into_database(
                &database_cloned.as_ref().unwrap().clone(),
                &unsent_logs_copy
            ).await;

            if result.is_err() {
                let error = result.err().unwrap();
                println!("Failed to store logs in the database, error: {}", error);
            } else {
                println!("Inserted {} logs into database", unsent_logs_copy.len());
            }
        }
    }

    async fn delete_old_logs_from_database(database: &Arc<Database>) -> anyhow::Result<u64> {
        let query = r#"
            DELETE
            FROM logs
            WHERE id IN (
                SELECT id
                FROM logs
                WHERE log_time < $1
                ORDER BY log_time DESC
            )
        "#;

        let connection = database.connection().await?;
        let statement = connection.prepare(query).await?;

        let date = Utc::now() - chrono::Duration::days(14);
        let deleted = connection.execute(&statement, &[&date]).await?;

        return Ok(deleted);
    }

    async fn store_logs_into_database(
        database: &Arc<Database>,
        unsent_logs: &Vec<LogLine>
    ) -> anyhow::Result<()> {
        if unsent_logs.is_empty() {
            return Ok(());
        }

        let mut connection = database.connection().await?;
        let transaction = connection.transaction().await?;

        let query = r#"
            INSERT INTO logs(
                log_time,
                log_level,
                target,
                message
            )
            VALUES ($1, $2, $3, $4)
        "#;

        for unsent_log in unsent_logs {
            transaction.execute(
                query,
                &[
                    &unsent_log.date_time,
                    &Self::log_level_to_string(&unsent_log.log_level),
                    &unsent_log.target,
                    &unsent_log.arguments
                ]
            ).await?;
        }

        transaction.commit().await?;
        return Ok(());
    }

    fn log_level_to_string(log_level: &LogLevel) -> &str {
        return match log_level {
            LogLevel::Error => "E",
            LogLevel::Warn => "W",
            LogLevel::Info => "I",
        };
    }

}

#[repr(usize)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum LogLevel {
    Error = 1,
    Warn,
    Info,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Error => write!(f, "E")?,
            LogLevel::Warn => write!(f, "W")?,
            LogLevel::Info => write!(f, "I")?,
        }

        return Ok(());
    }
}

#[derive(Clone)]
struct LogLine {
    date_time: DateTime<Utc>,
    log_level: LogLevel,
    target: String,
    arguments: String,
    thread_id: u64
}

#[macro_export(local_inner_macros)]
macro_rules! log {
    // log!(target: "my_target", Level::Info; "a {} event", "log");
    (target: $target:expr, $lvl:expr, $($arg:tt)+) => ({
        let lvl = $lvl;

        $crate::helpers::logger::__private_api_log(
            __log_format_args!($($arg)+),
            lvl,
            &($target, __log_module_path!(), __log_file!(), __log_line!()),
        );
    });

    // log!(Level::Info, "a log event")
    ($lvl:expr, $($arg:tt)+) => (log!(target: __log_module_path!(), $lvl, $($arg)+));
}

#[macro_export(local_inner_macros)]
macro_rules! error {
    // error!("a {} event", "log")
    ($($arg:tt)+) => (log!(crate::helpers::logger::LogLevel::Error, $($arg)+))
}

#[macro_export(local_inner_macros)]
macro_rules! warn {
    // info!("a {} event", "log")
    ($($arg:tt)+) => (log!(crate::helpers::logger::LogLevel::Warn, $($arg)+))
}

#[macro_export(local_inner_macros)]
macro_rules! info {
    // info!("a {} event", "log")
    ($($arg:tt)+) => (log!(crate::helpers::logger::LogLevel::Info, $($arg)+))
}

#[macro_export]
macro_rules! __log_format_args {
    ($($args:tt)*) => {
        format_args!($($args)*)
    };
}

#[macro_export]
macro_rules! __log_module_path {
    () => {
        module_path!()
    };
}

#[macro_export]
macro_rules! __log_file {
    () => {
        file!()
    };
}

#[macro_export]
macro_rules! __log_line {
    () => {
        line!()
    };
}

pub fn __private_api_log(
    args: fmt::Arguments,
    level: LogLevel,
    &(target, _module_path, _file, _line): &(&str, &'static str, &'static str, u32)
) {
    let thread_id = std::thread::current().id().as_u64().get();

    let log_line = LogLine {
        date_time: Utc::now(),
        log_level: level,
        target: target.to_string(),
        arguments: args.to_string(),
        thread_id: thread_id
    };

    let logger = logger();
    let _ = logger.sender.send(log_line);
}