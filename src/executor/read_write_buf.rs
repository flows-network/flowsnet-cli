use std::io::{Result, Write};
use std::sync::{Arc, RwLock};

pub struct WriteBuf {
    rc_buf: Arc<RwLock<Vec<u8>>>,
}

impl Write for WriteBuf {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.rc_buf.write().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct ReadWriteBuf {
    inner_buf: Arc<RwLock<Vec<u8>>>,
}

impl ReadWriteBuf {
    pub fn new() -> Self {
        Self {
            inner_buf: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn get_write_buf(&self) -> WriteBuf {
        WriteBuf {
            rc_buf: Arc::clone(&self.inner_buf),
        }
    }

    pub fn read_all(&self) -> String {
        match self.inner_buf.read() {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(_) => String::new(),
        }
    }
}
