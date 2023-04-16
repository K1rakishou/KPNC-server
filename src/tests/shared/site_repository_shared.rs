use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::model::repository::site_repository::SiteRepository;

static SITE_REPOSITORY: OnceCell<Arc<SiteRepository>> = OnceCell::new();

pub fn site_repository() -> &'static Arc<SiteRepository> {
    return SITE_REPOSITORY.get().unwrap();
}

pub async fn ctor() {
    let _ = SITE_REPOSITORY.set(Arc::new(SiteRepository::new()));
}

pub async fn dtor() {
    
}