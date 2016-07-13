
pub struct CacheEntry {
    // Raw data being cached
    pub data: Vec<u8>,
    // Indicates if this cache entry has every been warmed (e.g. read from API or populated by write)
    pub warm: bool,
    // Indicates if the data is in sync with the API (false implies we should persists)
    pub sync: bool,
    // Number of open handles to this CacheEntry
    handles: u32,
}

impl CacheEntry {
    pub fn new() -> CacheEntry {
        CacheEntry {
            data: Vec::new(),
            warm: false,
            sync: false,
            handles: 0,
        }
    }

    pub fn set<I: Into<Vec<u8>>>(&mut self, data: I) {
        self.sync = false;
        self.warm = true;
        self.data = data.into();
    }

    pub fn write(&mut self, offset: u64, data: &[u8]) {
        self.sync = false;
        self.warm = true;
        let end = offset as usize + data.len();
        self.data.resize(end, 0);
        println!("write(offset={}, data.len={}, end={})", offset, data.len(), end);
        self.data[(offset as usize)..end].copy_from_slice(data);
    }

    pub fn released(&mut self) -> u32 {
        self.handles = self.handles - 1;
        self.handles
    }

    pub fn opened(&mut self) -> u32 {
        self.handles = self.handles + 1;
        self.handles
    }
}

