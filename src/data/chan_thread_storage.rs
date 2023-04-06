use lazy_static::lazy_static;
use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::data::chan::ChanThread;

lazy_static! {
    static ref accounts: RwLock<HashMap<u64, ChanThread>> = RwLock::new(HashMap::with_capacity(1024));
}