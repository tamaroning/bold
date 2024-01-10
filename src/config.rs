pub const PAGE_SIZE: u64 = 0x1000;

pub struct Config {
    pub image_base: u64,
}

impl Config {
    pub fn new() -> Config {
        Config {
            image_base: 0x400000,
        }
    }
}
