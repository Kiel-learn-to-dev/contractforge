use fs4::{FileExt, TryLockError};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use crate::DbError;

#[derive(Debug)]
pub struct WriterLock {
    file: File,
}

impl WriterLock {
    pub fn acquire(root: &Path) -> Result<Self, DbError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(".cf_writer.lock"))
            .map_err(|error| DbError::Schema(error.to_string()))?;
        FileExt::try_lock(&file).map_err(|error| match error {
            TryLockError::WouldBlock => DbError::Locked,
            TryLockError::Error(source) => DbError::Schema(source.to_string()),
        })?;
        Ok(Self { file })
    }

    pub fn file(&self) -> &File {
        &self.file
    }
}
