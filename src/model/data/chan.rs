use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteDescriptor {
    site_name: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogDescriptor {
    site_descriptor: SiteDescriptor,
    board_code: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThreadDescriptor {
    catalog_descriptor: CatalogDescriptor,
    thread_no: u64
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostDescriptor {
    thread_descriptor: ThreadDescriptor,
    post_no: u64,
    post_sub_no: u64
}

#[derive(Debug)]
pub struct ChanThread {
    thread_descriptor: ThreadDescriptor,
    is_alive: bool
}

impl CatalogDescriptor {
    pub fn new(site_name: String, board_code: String) -> CatalogDescriptor {
        return CatalogDescriptor {
            site_descriptor: SiteDescriptor { site_name },
            board_code
        }
    }

    pub fn from_site_descriptor(
        site_descriptor: SiteDescriptor,
        board_code: String
    ) -> CatalogDescriptor {
        return CatalogDescriptor {
            site_descriptor,
            board_code
        }
    }
}

impl ThreadDescriptor {
    pub fn new(
        site_name: String,
        board_code: String,
        thread_no: u64
    ) -> ThreadDescriptor {
        let site_descriptor = SiteDescriptor { site_name };
        let catalog_descriptor = CatalogDescriptor { site_descriptor, board_code };

        return ThreadDescriptor {
            catalog_descriptor,
            thread_no
        }
    }

    pub fn from_catalog_descriptor(
        catalog_descriptor: CatalogDescriptor,
        thread_no: u64
    ) -> ThreadDescriptor {
        return ThreadDescriptor {
            catalog_descriptor,
            thread_no
        }
    }
}

impl PostDescriptor {
    pub fn new(
        site_name: String,
        board_code: String,
        thread_no: u64,
        post_no: u64,
        post_sub_no: u64
    ) -> PostDescriptor {
        let site_descriptor = SiteDescriptor { site_name };
        let catalog_descriptor = CatalogDescriptor { site_descriptor, board_code };
        let thread_descriptor = ThreadDescriptor { catalog_descriptor, thread_no };

        return PostDescriptor {
            thread_descriptor,
            post_no,
            post_sub_no
        }
    }

    pub fn from_thread_descriptor(
        thread_descriptor: ThreadDescriptor,
        post_no: u64
    ) -> PostDescriptor {
        return PostDescriptor {
            thread_descriptor,
            post_no,
            post_sub_no: 0u64
        }
    }
}