use std::fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteDescriptor {
    pub site_name: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogDescriptor {
    pub site_descriptor: SiteDescriptor,
    pub board_code: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThreadDescriptor {
    pub catalog_descriptor: CatalogDescriptor,
    pub thread_no: u64
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostDescriptor {
    pub thread_descriptor: ThreadDescriptor,
    pub post_no: u64,
    pub post_sub_no: u64
}

impl Display for SiteDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.site_name)?;

        return Ok(());
    }
}

impl Display for CatalogDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/", self.site_name())?;
        write!(f, "{}", self.board_code())?;

        return Ok(());
    }
}

impl CatalogDescriptor {
    pub fn site_name(&self) -> &String {
        return &self.site_descriptor.site_name;
    }

    pub fn board_code(&self) -> &String {
        return &self.board_code;
    }

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

impl Display for ThreadDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/", self.site_name())?;
        write!(f, "{}/", self.board_code())?;
        write!(f, "{}", self.thread_no)?;

        return Ok(());
    }
}

impl ThreadDescriptor {
    pub fn site_name(&self) -> &String {
        return &self.catalog_descriptor.site_descriptor.site_name;
    }

    pub fn board_code(&self) -> &String {
        return &self.catalog_descriptor.board_code;
    }

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

    pub fn from_row(row: &Row) -> ThreadDescriptor {
        let site_name: String = row.get(0);
        let board_code: String = row.get(1);
        let thread_no: i64 = row.get(2);

        return ThreadDescriptor::new(site_name, board_code, thread_no as u64);
    }
}

impl Display for PostDescriptor {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/", self.site_name())?;
        write!(f, "{}/", self.board_code())?;
        write!(f, "{}/", self.thread_no())?;
        write!(f, "{}/", self.post_no)?;
        write!(f, "{}", self.post_sub_no)?;
        
        return Ok(());
    }
}

impl PostDescriptor {
    pub fn site_name(&self) -> &String {
        return &self.thread_descriptor.site_name();
    }

    pub fn board_code(&self) -> &String {
        return &self.thread_descriptor.board_code();
    }

    pub fn thread_no(&self) -> u64 {
        return self.thread_descriptor.thread_no
    }

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