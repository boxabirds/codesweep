fn main() {
    // The command surface arrives with the port of the commands themselves.
    // Until then this exists so the crate builds as a binary and there is
    // somewhere obvious for the commands to land.
    eprintln!("resweep: the Rust port is not finished. Use bin/resweep.");
    std::process::exit(1);
}
