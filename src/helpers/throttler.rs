use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::Duration;

use lazy_static::lazy_static;
use tokio::sync::RwLock;

use crate::{info, warn};
use crate::router::TestContext;

lazy_static! {
    static ref VISITORS: RwLock<lru::LruCache<String, VisitorInfo>> =
        RwLock::new(lru::LruCache::new(NonZeroUsize::new(4096).unwrap()));

    static ref REQUEST_LIMITS: RwLock<HashMap<String, usize>> = RwLock::new(init_request_limits());
}

struct VisitorInfo {
    requests_counter: HashMap<String, usize>
}

impl VisitorInfo {
    pub fn new() -> VisitorInfo {
        return VisitorInfo {
            requests_counter: HashMap::with_capacity(16)
        }
    }
}

pub async fn cleanup_task() {
    info!("cleanup_task() start");

    loop {
        info!("cleanup_task() cleaning up...");

        {
            let mut visitors_locked = VISITORS.write().await;
            for (_, visitor_info) in visitors_locked.iter_mut() {
                for (_, requests_count) in visitor_info.requests_counter.iter_mut() {
                    *requests_count = 0;
                }
            }
        }

        info!("cleanup_task() cleaning up... done, waiting...");
        tokio::time::sleep(Duration::from_secs(60)).await;
        info!("cleanup_task() waiting... done");
    }

    info!("cleanup_task() end");
}

pub async fn can_proceed(
    test_context: Option<TestContext>,
    path: String,
    remote_address: &String
) -> anyhow::Result<bool> {
    if test_context.is_some() && !test_context.unwrap().enable_throttler {
        return Ok(true);
    }

    let ip_address = extract_ip_address(remote_address);

    let counter = {
        let mut visitors_locked = VISITORS.write().await;
        let visitor_info = visitors_locked.get_or_insert_mut(ip_address, || VisitorInfo::new());
        let counter = visitor_info.requests_counter.entry(path.clone()).or_insert(0);

        *counter += 1;
        counter.clone()
    };

    let can_proceed = {
        let request_limits_locked = REQUEST_LIMITS.write().await;
        let limit_for_this_path = request_limits_locked.get(&path);

        if limit_for_this_path.is_none() {
            warn!("Path \'{}\' has no request limit!!! Passing all requests!", path);
            true
        } else {
            let limits = limit_for_this_path.unwrap();
            counter <= *limits
        }
    };

    return Ok(can_proceed);
}

fn init_request_limits() -> HashMap<String, usize> {
    let mut result_map = HashMap::<String, usize>::new();

    // All limits are per minute.
    result_map.insert("create_account".to_string(), 5);
    result_map.insert("update_account_expiry_date".to_string(), 5);
    result_map.insert("update_firebase_token".to_string(), 5);
    result_map.insert("get_account_info".to_string(), 15);
    result_map.insert("watch_post".to_string(), 10);
    result_map.insert("".to_string(), 30);
    result_map.insert("favicon.ico".to_string(), 30);

    return result_map;
}

fn extract_ip_address(remote_address: &String) -> String {
    let index = remote_address.find(":");
    if index.is_none() {
        return remote_address.to_string()
    }

    let index = index.unwrap();
    return remote_address[0..index].to_string()
}

#[test]
fn test() {
    let ip = extract_ip_address(&String::from("127.0.0.1:50016"));
    assert_eq!("127.0.0.1", ip.as_str());

    let ip = extract_ip_address(&String::from("127.0.0.1"));
    assert_eq!("127.0.0.1", ip.as_str());
}