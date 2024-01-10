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
