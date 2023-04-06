use crate::model::data::chan::ChanThread;

struct ThreadWatcher {

}

impl ThreadWatcher {
    pub fn new() -> ThreadWatcher {
        return ThreadWatcher {};
    }

    pub async fn process_threads(chan_threads: &Vec<ChanThread>) -> anyhow::Result<()> {
        todo!()
    }

}