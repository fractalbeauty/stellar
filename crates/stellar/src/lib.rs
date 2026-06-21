pub use stellar_sync as sync;

uniffi::setup_scaffolding!();

pub fn run() {
    println!("Hello, world!");
}

#[uniffi::export]
fn add(a: u32, b: u32) -> u32 {
    a + b
}
